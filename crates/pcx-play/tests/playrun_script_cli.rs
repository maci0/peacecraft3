//! `playrun-script` refusal contract.
//!
//! The probe compiles and executes local extracted sources, so its refusals
//! are the only part that can be checked without the original game assets. No
//! original source is copied into the repository and none is needed here: the
//! arguments are wrong, or the files are missing, or the bytes are not JASS.

use std::path::PathBuf;
use std::process::Command;

fn playrun_script(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_playrun-script"))
        .args(args)
        .output()
        .expect("playrun-script must run")
}

#[test]
fn a_wrong_argument_count_reports_usage_on_stderr_with_silent_stdout() {
    // The probe takes the edition, version, asset source, three script files
    // and an optional WTS file.
    for args in [
        Vec::<&str>::new(),
        vec!["edition"],
        vec!["edition", "version", "source", "common.j", "Blizzard.j"],
        vec![
            "edition",
            "version",
            "source",
            "common.j",
            "Blizzard.j",
            "war3map.j",
            "war3map.wts",
            "extra",
        ],
    ] {
        let result = playrun_script(&args);
        assert_eq!(result.status.code(), Some(2), "args={args:?}");
        assert!(result.stdout.is_empty(), "args={args:?}");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains("usage:"), "args={args:?} stderr={stderr:?}");
    }
}

#[test]
fn an_unreadable_source_is_refused_on_stderr_with_silent_stdout() {
    let missing = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("playrun-script-absent.j");
    let missing = missing.to_str().unwrap();
    let result = playrun_script(&["edition", "version", "source", missing, missing, missing]);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("No such file"),
        "stderr={stderr:?} must name the unreadable source"
    );
}

#[test]
fn bytes_that_are_not_jass_fail_closed_on_stderr_with_silent_stdout() {
    let broken = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("playrun-script-broken.j");
    std::fs::write(&broken, "not jass at all {{{\n").unwrap();
    let broken = broken.to_str().unwrap();
    let result = playrun_script(&["edition", "version", "source", broken, broken, broken]);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains(broken),
        "stderr={stderr:?} must name the source that failed"
    );
}

//! The `pcx-diff` binary, driven end to end: real files in, stdout out.

use std::path::PathBuf;
use std::process::Command;

use pcx_play::{AssetMode, RunConfig, run, smoke, smoke_config};

fn write_trace(name: &str, bytes: &[u8]) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, bytes).expect("trace file must write");
    path
}

fn diff(left: &std::path::Path, right: &std::path::Path) -> (String, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_pcx-diff"))
        .arg(left)
        .arg(right)
        .output()
        .expect("pcx-diff must run");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().expect("pcx-diff must exit"),
    )
}

#[test]
fn the_cli_reports_unchanged_and_the_first_divergence() {
    let a = run(&smoke(), &smoke_config()).expect("smoke must run");
    let b = run(&smoke(), &smoke_config()).expect("smoke must run");
    let config = RunConfig {
        asset_mode: AssetMode::Reforged,
        ..smoke_config()
    };
    let c = run(&smoke(), &config).expect("smoke must run");

    let a_path = write_trace("a.trace", &a.encode());
    let b_path = write_trace("b.trace", &b.encode());
    let c_path = write_trace("c.trace", &c.encode());

    let (stdout, stderr, code) = diff(&a_path, &b_path);
    assert_eq!(code, 0);
    assert_eq!(stdout, "unchanged\n");
    assert_eq!(stderr, "");

    let (stdout, stderr, code) = diff(&a_path, &c_path);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        "diverge tick=header field=asset-mode left=vanilla right=reforged\n"
    );
    assert_eq!(stderr, "");

    let (stdout, _, code) = diff(&a_path, &a_path);
    assert_eq!(code, 0);
    assert_eq!(stdout, "unchanged\n");
}

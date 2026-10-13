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

#[test]
fn script_traces_compare_and_incompatible_schemas_are_refused() {
    use pcx_play::campaign::{ChapterTrace, NativeRecord};
    use pcx_play::scripts::AssetIdentity;
    use pcx_script::Value;
    let a = ChapterTrace {
        chapter: "synthetic".into(),
        identity: AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture".into(),
            source: "memory".into(),
        },
        sources: Vec::new(),
        ticks: 1,
        calls: Vec::new(),
        natives: vec![NativeRecord {
            sequence: 0,
            tick: Some(0),
            name: "Observed".into(),
            args: Vec::new(),
            result: Value::Bool(true),
        }],
        gates: Vec::new(),
    };
    let mut b = a.clone();
    b.natives[0].result = Value::Bool(false);
    let a_path = write_trace("script-a.trace", &a.encode());
    let b_path = write_trace("script-b.trace", &b.encode());
    assert_eq!(diff(&a_path, &a_path), ("unchanged\n".into(), "".into(), 0));
    let (stdout, stderr, code) = diff(&a_path, &b_path);
    assert_eq!(code, 1);
    assert_eq!(stderr, "");
    assert!(
        stdout.contains("tick=0 field=script.event[0].native:Observed"),
        "{stdout}"
    );
    let unit = write_trace(
        "unit-for-schema.trace",
        &run(&smoke(), &smoke_config()).unwrap().encode(),
    );
    let (_, stderr, code) = diff(&a_path, &unit);
    assert_eq!(code, 2);
    assert!(stderr.contains("trace schemas"));
    let malformed = write_trace("invalid-script.trace", b"pcx-script-trace 2\n");
    assert_eq!(diff(&a_path, &malformed).2, 2);
}

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

fn minimize_files(prefix: &str, right_map: &str, expectation: &str) -> (PathBuf, PathBuf) {
    let scenario = |map: &str, x: i32, expectation: &str| {
        format!(
            "pcx-rpl 1\nscenario minimize-case\nmap {map}\nticks 2\nseed 99\nasset-mode vanilla\nspawn 0 0 0\nat 0 move 0 move {x} 0\nexpect 0 units 1\n{expectation}"
        )
    };
    let left = write_trace(
        &format!("{prefix}-left.rpl"),
        scenario("synthetic.map", 1, "").as_bytes(),
    );
    let right = write_trace(
        &format!("{prefix}-right.rpl"),
        scenario(right_map, 2, expectation).as_bytes(),
    );
    (left, right)
}

fn minimize_command(
    left: &std::path::Path,
    right: &std::path::Path,
    args: &[&str],
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pcx-diff"))
        .arg("minimize")
        .arg(left)
        .arg(right)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn minimize_cli_runs_fresh_pairs_and_reports_corpus_minimum() {
    let (left, right) = minimize_files("minimize-positive", "synthetic.map", "");
    let output = minimize_command(&left, &right, &["8", "20", "20", "6", "3", "6", "8", "1"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("minimum-seed=1 original-seed=8 oracle-evaluations=4 inspections=6 status=CorpusExhausted\n"), "{stdout}");
    assert!(stdout.contains("field=unit[0].x"), "{stdout}");
    assert!(output.stderr.is_empty());

    let output = minimize_command(&left, &right, &["8", "1", "20", "1"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains(
        "minimum-seed=8 original-seed=8 oracle-evaluations=1 inspections=1 status=BudgetExhausted"
    ));
}

#[test]
fn minimize_cli_refuses_nonreproduction_and_incompatible_headers() {
    let (left, right) = minimize_files("minimize-incompatible", "different.map", "");
    let output = minimize_command(&left, &right, &["8", "10", "10", "1"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("map identity")
    );
    assert!(output.stdout.is_empty());

    let output = minimize_command(&left, &left, &["8", "10", "10", "1"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("did not reproduce")
    );
}

#[test]
fn minimize_cli_preserves_oracle_errors_and_rejects_bad_budgets() {
    let (left, right) = minimize_files("minimize-errors", "synthetic.map", "expect 0 units 99\n");
    let output = minimize_command(&left, &right, &["8", "10", "10", "1"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("expectation")
    );
    assert!(output.stdout.is_empty());

    for args in [["8", "0", "10"], ["8", "10", "0"], ["bad-seed", "10", "10"]] {
        let output = minimize_command(&left, &right, &args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

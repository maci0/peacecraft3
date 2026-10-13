use std::path::PathBuf;
use std::process::Command;
#[test]
fn checked_scenario_cli_baseline_determinism_and_work_limit() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/play/synthetic/ordering/same-tick.rpl");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("playrun-ordering.trace");
    let baseline =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("playrun-ordering-baseline.trace");
    let call = |options: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_playrun"))
            .arg(&input)
            .arg(&output)
            .args(options)
            .output()
            .unwrap()
    };
    let result = call(&["--runs", "100"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("runs=100"));
    std::fs::copy(&output, &baseline).unwrap();
    assert!(
        call(&["--baseline", baseline.to_str().unwrap()])
            .status
            .success()
    );
    let work_failure = call(&["--max-work", "1"]);
    assert_eq!(work_failure.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&work_failure.stderr).contains("work"));
    let mut trace = pcx_play::Trace::decode(&std::fs::read(&baseline).unwrap()).unwrap();
    trace.header.seed += 1;
    std::fs::write(&baseline, trace.encode()).unwrap();
    let diff = call(&["--baseline", baseline.to_str().unwrap()]);
    assert_eq!(diff.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&diff.stderr).contains("field=seed"));
}

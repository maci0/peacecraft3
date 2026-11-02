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
    assert!(String::from_utf8_lossy(&result.stderr).contains("runs=100")); // `passed` is a diagnostic; the trace file is the data artifact.
    assert!(
        result.stdout.is_empty(),
        "stdout carries no data channel here"
    );
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

#[test]
fn malformed_invocations_are_refused_on_stderr_with_silent_stdout() {
    let scenario = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/play/synthetic/ordering/same-tick.rpl");
    let scenario = scenario.to_str().unwrap();
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let output = tmp.join("playrun-refusal.trace");
    let output = output.to_str().unwrap();
    let baseline = tmp.join("playrun-refusal-baseline.trace");
    let baseline = baseline.to_str().unwrap();
    let other = tmp.join("playrun-refusal-other.trace");
    let other = other.to_str().unwrap();
    let absent = tmp.join("playrun-refusal-absent.rpl");
    let absent = absent.to_str().unwrap();
    let malformed = tmp.join("playrun-refusal-malformed.rpl");
    std::fs::write(&malformed, "pcx-rpl 1\n").unwrap();
    let malformed = malformed.to_str().unwrap();

    // Every refusal names what went wrong on stderr, leaves stdout unused and
    // exits 2; a run that never starts is not a divergence, so it must not
    // borrow the divergence exit code.
    let cases: [(Vec<&str>, &str); 11] = [
        (vec![], "usage:"),
        (vec![scenario], "usage:"),
        (vec![scenario, output, "--bogus", "x"], "unknown option"),
        (vec![scenario, output, "--runs"], "usage:"),
        (
            vec![scenario, output, "--runs", "0"],
            "run count must be positive",
        ),
        (vec![scenario, output, "--runs", "abc"], "invalid run count"),
        (
            vec![
                scenario,
                output,
                "--baseline",
                baseline,
                "--baseline",
                other,
            ],
            "duplicate baseline",
        ),
        (vec![scenario, scenario], "output must differ"),
        (vec![absent, output], "No such file"),
        (vec![malformed, output], "required field is missing"),
        (
            vec![scenario, output, "--max-work", "abc"],
            "invalid max work",
        ),
    ];
    for (args, expected) in cases {
        let result = Command::new(env!("CARGO_BIN_EXE_playrun"))
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "args={args:?}");
        assert!(result.stdout.is_empty(), "args={args:?}");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains(expected),
            "args={args:?} stderr={stderr:?} must name {expected:?}"
        );
    }
}

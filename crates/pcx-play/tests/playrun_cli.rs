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
    let original_seed = trace.header.seed;
    trace.header.seed += 1;
    std::fs::write(&baseline, trace.encode()).unwrap();
    // Remove the previous output so the retained candidate cannot be a
    // leftover from the earlier successful run.
    std::fs::remove_file(&output).unwrap();
    let diff = call(&["--baseline", baseline.to_str().unwrap()]);
    assert_eq!(diff.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&diff.stderr).contains("field=seed"));
    // A comparison failure retains the candidate trace for diagnosis, and it
    // is the candidate, not the baseline it was compared against.
    let retained = pcx_play::Trace::decode(&std::fs::read(&output).unwrap()).unwrap();
    assert_eq!(retained.header.seed, original_seed);
    let source = std::fs::read_to_string(&input).unwrap();
    let rpl = pcx_play::Rpl::parse(&source).unwrap();
    assert_eq!(retained.encode(), rpl.run().unwrap().encode());
}

#[test]
fn the_work_gate_counts_the_shared_work_model() {
    // The gate's work number is the generator's model, so the command cannot
    // drift from the ceiling the generator enforces. The checked-in corpus
    // entry is the one the generation note names: 12 ticks, 8 units and 24
    // actions, so 12 * (8 + 1) + 24 = 132 per run.
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/play/synthetic/generated-movement/seed-0.rpl");
    let source = std::fs::read_to_string(&input).unwrap();
    let rpl = pcx_play::Rpl::parse(&source).unwrap();
    let per_run = pcx_play::generator::work_estimate(
        rpl.scenario.ticks,
        rpl.scenario.spawns.len(),
        rpl.scenario.script.len(),
    );
    assert_eq!(per_run, 132, "the checked-in entry's work");
    let runs = 100u64;
    let limit = u64::try_from(per_run * u128::from(runs)).unwrap();
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("playrun-work.trace");
    let call = |options: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_playrun"))
            .arg(&input)
            .arg(&output)
            .args(options)
            .output()
            .unwrap()
    };
    // Exactly at the model's total the run is admitted.
    let admitted = call(&["--runs", "100", "--max-work", &limit.to_string()]);
    assert!(
        admitted.status.success(),
        "{}",
        String::from_utf8_lossy(&admitted.stderr)
    );
    // One work unit below it the run is refused, and the refusal names the
    // model's own number rather than a count of its own.
    let refused = call(&["--runs", "100", "--max-work", &(limit - 1).to_string()]);
    assert_eq!(refused.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains(&format!("work {limit} exceeds limit {}", limit - 1)),
        "{stderr}"
    );
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

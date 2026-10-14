use pcx_play::{RunError, Scenario, first_divergence, run, smoke, smoke_config};

fn scenario(ticks: u64) -> Scenario {
    Scenario {
        name: "capacity-fixture".into(),
        map: "synthetic.capacity".into(),
        ticks,
        spawns: vec![],
        script: vec![],
    }
}

#[test]
fn unrepresentable_trace_layout_is_a_typed_error_before_allocation() {
    let result = run(&scenario(u64::MAX), &smoke_config());
    assert!(
        matches!(result, Err(RunError::TraceCapacity { ticks: u64::MAX, ref scenario }) if scenario == "capacity-fixture")
    );
}

#[test]
fn zero_ticks_and_normal_runs_preserve_canonical_traces() {
    let empty = run(&scenario(0), &smoke_config()).unwrap();
    assert_eq!(empty.header.tick_count, 0);
    assert!(empty.ticks.is_empty());
    let first = run(&smoke(), &smoke_config()).unwrap();
    let next = run(&smoke(), &smoke_config()).unwrap();
    assert_eq!(first.encode(), next.encode());
    assert!(first_divergence(&first, &next).is_none());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cli_reports_capacity_failure_and_preserves_existing_output() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    let input = dir.join("capacity-fixture.rpl");
    let output = dir.join("capacity-fixture.trace");
    std::fs::write(&input, "pcx-rpl 1\nscenario capacity-fixture\nmap synthetic.capacity\nticks 18446744073709551615\nseed 1\nasset-mode vanilla\nexpect 0 units 0\n").unwrap();
    std::fs::write(&output, b"existing-output").unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_playrun"))
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("vector layout"));
    assert_eq!(std::fs::read(&output).unwrap(), b"existing-output");
}

#[cfg(target_pointer_width = "32")]
#[test]
fn tick_count_larger_than_usize_is_not_truncated() {
    let ticks = u64::from(u32::MAX) + 1;
    assert!(
        matches!(run(&scenario(ticks), &smoke_config()), Err(RunError::TraceCapacity { ticks: got, .. }) if got == ticks)
    );
}

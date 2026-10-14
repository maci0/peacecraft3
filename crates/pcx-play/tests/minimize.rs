//! Tests for bounded differential seed minimization.
//!
//! Validates:
//! 1. `failingseedchecked`: Original seed failure verification before search.
//! 2. `earlyunrelatedfield`: Rejecting candidate divergences on unrelated fields.
//! 3. `budgetexhaustion`: Halting search when maximum evaluations budget is reached.
//! 4. `errorpropagation`: Propagating oracle errors, stale traces, and incompatible headers.
//! 5. `dedup`: Deterministic candidate deduplication without redundant evaluations.
//! 6. `numericalmininfinitecorpus`: Bounded search over infinite candidate stream without hanging.
//! 7. `genuinetraceoracle`: Dual-variant execution with real pcx_play runner and first_divergence.
//! 8. `unsupportedversionrejected`: Unsupported trace format versions are refused.
//! 9. `noncanonicalfixrowsrejected`: Noncanonical fix rows (duplicate/non-ascending/invalid ids) refused.
//! 10. `inspectionbudgetbeforeadvancement`: Budget check precedes iterator advancement.
//!
//! Caveat: The inspection budget bounds iterations made by this search module.
//! It does not preempt a caller's blocking iterator, predicate, or execution oracle.
//! Such callbacks still require caller-controlled termination. An exhausted budget
//! must not be presented as proof that the entire finite corpus has been inspected.

use pcx_play::minimize;

use std::sync::atomic::{AtomicUsize, Ordering};

use minimize::{
    FailureSignature, MinimizationConfig, MinimizationError, MinimizationStatus, TracePairError,
    minimize_differential_seed, search_scope,
};
use pcx_play::{
    AssetMode, Scenario, ScheduledAction, ScriptedAction, SpawnSpec, TickRecord, TimerEvent,
    TimerEventKind, Trace, TraceHeader, UnitState, smoke_config,
};
use pcx_sim::{Command, FX_ONE};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
enum SyntheticOracleError {
    #[error("simulated oracle execution error: {0}")]
    Failure(String),
}

/// Creates a minimal valid synthetic trace for differential testing.
fn make_synthetic_trace(
    scenario: &str,
    map: &str,
    seed: u64,
    tick_count: usize,
    unit_x: i32,
    timer_name: &str,
) -> Trace {
    Trace {
        header: TraceHeader {
            format_version: 1,
            scenario: scenario.to_string(),
            map: map.to_string(),
            asset_mode: AssetMode::Vanilla,
            seed,
            fix_rows: Vec::new(),
            tick_count: tick_count as u64,
        },
        ticks: (0..tick_count)
            .map(|i| {
                let mut units = Vec::new();
                let mut timers = Vec::new();
                if i == 10 {
                    units.push(UnitState {
                        id: 0,
                        owner: 0,
                        x: unit_x,
                        y: 0,
                        target: None,
                    });
                }
                if i == 2 && !timer_name.is_empty() {
                    timers.push(TimerEvent {
                        seq: 0,
                        kind: TimerEventKind::Fire,
                        name: timer_name.to_string(),
                    });
                }
                TickRecord {
                    tick: i as u64,
                    units,
                    rng_draws: Vec::new(),
                    timers,
                }
            })
            .collect(),
    }
}

/// `failingseedchecked`: Search must refuse to run if the original seed does not
/// reproduce the expected failure signature.
#[test]
fn failingseedchecked() {
    let original_seed = 42u64;

    let oracle_passing = |seed: u64| {
        let left = make_synthetic_trace("synth", "synth.map", seed, 15, 100, "");
        let right = make_synthetic_trace("synth", "synth.map", seed, 15, 100, "");
        Ok::<_, SyntheticOracleError>((left, right))
    };

    let config_a = MinimizationConfig::new(original_seed, vec![10, 5], 10);
    let result_a = minimize_differential_seed(config_a, oracle_passing);

    match result_a {
        Err(MinimizationError::OriginalSeedDidNotDiverge { seed }) => {
            assert_eq!(seed, original_seed);
        }
        other => panic!("expected OriginalSeedDidNotDiverge, got {other:?}"),
    }

    let oracle_mismatched = |seed: u64| {
        let left = make_synthetic_trace("synth", "synth.map", seed, 15, 100, "timer_event_a");
        let right = make_synthetic_trace("synth", "synth.map", seed, 15, 100, "timer_event_b");
        Ok::<_, SyntheticOracleError>((left, right))
    };

    let config_b = MinimizationConfig::new(original_seed, vec![10, 5], 10).with_predicate(
        |cand: &pcx_play::Divergence, _orig: &pcx_play::Divergence| {
            cand.field.starts_with("unit[0]")
        },
    );
    let result_b = minimize_differential_seed(config_b, oracle_mismatched);

    match result_b {
        Err(MinimizationError::OriginalSeedMismatchedSignature { seed, divergence }) => {
            assert_eq!(seed, original_seed);
            assert!(divergence.field.starts_with("timer[0]"));
        }
        other => panic!("expected OriginalSeedMismatchedSignature, got {other:?}"),
    }

    let config_c = MinimizationConfig::new(original_seed, vec![10, 5], 0);
    let result_c = minimize_differential_seed(config_c, oracle_passing);
    assert!(matches!(
        result_c,
        Err(MinimizationError::ZeroOracleEvaluationBudget)
    ));
}

/// `earlyunrelatedfield`: Candidate divergences on unrelated fields must not count.
#[test]
fn earlyunrelatedfield() {
    let original_seed = 100u64;

    let oracle = |seed: u64| {
        let (left_x, right_x, left_t, right_t) = match seed {
            100 => (100, 200, "", ""),
            50 => (100, 100, "early_a", "early_b"),
            30 => (100, 200, "", ""),
            10 => (100, 100, "", ""),
            _ => (100, 100, "", ""),
        };
        let left = make_synthetic_trace("synth", "synth.map", seed, 15, left_x, left_t);
        let right = make_synthetic_trace("synth", "synth.map", seed, 15, right_x, right_t);
        Ok::<_, SyntheticOracleError>((left, right))
    };

    let candidates = vec![50u64, 30, 10];
    let config = MinimizationConfig::new(original_seed, candidates.clone(), 10)
        .with_predicate(FailureSignature::SameField);
    let result = minimize_differential_seed(config, oracle).expect("minimization should succeed");

    assert_eq!(result.original_seed, 100);
    assert_eq!(result.minimum_seed, 30);
    assert_eq!(result.minimum_divergence.field, "unit[0].x");
    assert_eq!(result.oracle_evaluations, 4); // 100, 50, 30, 10
    assert!(result.is_complete());
    assert!(result.reduced());

    let config_tick = MinimizationConfig::new(original_seed, candidates.clone(), 10)
        .with_predicate(FailureSignature::SameTickAndField);
    let result_tick =
        minimize_differential_seed(config_tick, oracle).expect("minimization should succeed");
    assert_eq!(result_tick.minimum_seed, 30);

    let config_exact = MinimizationConfig::new(original_seed, candidates, 10)
        .with_predicate(FailureSignature::Exact);
    let result_exact =
        minimize_differential_seed(config_exact, oracle).expect("minimization should succeed");
    assert_eq!(result_exact.minimum_seed, 30);

    let seq: Vec<u64> = search_scope::sequential_small(5).collect();
    assert_eq!(seq, vec![0, 1, 2, 3, 4]);

    let std_scope: Vec<u64> = search_scope::standard_reduction_scope(32).collect();
    assert!(!std_scope.is_empty());
    assert!(std_scope.contains(&0));
    assert!(std_scope.contains(&16));
}

/// `budgetexhaustion`: Search must stop at budget, preserving best verified minimum.
#[test]
fn budgetexhaustion() {
    let original_seed = 100u64;
    let eval_count = AtomicUsize::new(0);

    let oracle = |seed: u64| {
        eval_count.fetch_add(1, Ordering::SeqCst);
        let left = make_synthetic_trace("synth", "synth.map", seed, 15, 100, "");
        let right = make_synthetic_trace("synth", "synth.map", seed, 15, 200, "");
        Ok::<_, SyntheticOracleError>((left, right))
    };

    let candidates = vec![80u64, 60, 40, 20, 10];
    // Budget test: oracle budget is 3; inspection budget is set higher so oracle budget is the binder.
    let config = MinimizationConfig::new(original_seed, candidates, 3).with_max_inspections(20);
    let result = minimize_differential_seed(config, oracle).expect("minimization should succeed");

    assert_eq!(result.oracle_evaluations, 3);
    assert_eq!(eval_count.load(Ordering::SeqCst), 3);
    assert_eq!(result.minimum_seed, 60);
    assert_eq!(result.status, MinimizationStatus::BudgetExhausted);
    assert!(result.is_oracle_budget_exhausted());
    assert!(!result.is_complete());
    assert!(result.reduced());
}

/// `errorpropagation`: Typed errors preserved, never treated as reproductions.
#[test]
fn errorpropagation() {
    let original_seed = 100u64;

    let oracle_failing_candidate = |seed: u64| {
        if seed == 50 {
            return Err(SyntheticOracleError::Failure("backend crash".to_string()));
        }
        let left = make_synthetic_trace("synth", "synth.map", seed, 15, 100, "");
        let right = make_synthetic_trace("synth", "synth.map", seed, 15, 200, "");
        Ok((left, right))
    };

    let config_a = MinimizationConfig::new(original_seed, vec![50u64], 5);
    let result_a = minimize_differential_seed(config_a, oracle_failing_candidate);

    match result_a {
        Err(MinimizationError::CandidateOracle {
            candidate_seed,
            source,
        }) => {
            assert_eq!(candidate_seed, 50);
            assert_eq!(
                source,
                SyntheticOracleError::Failure("backend crash".to_string())
            );
        }
        other => panic!("expected CandidateOracle error, got {other:?}"),
    }

    let oracle_stale_trace = |seed: u64| {
        let left = make_synthetic_trace("synth", "synth.map", seed, 15, 100, "");
        let stale_seed = if seed == 50 { 999 } else { seed };
        let right = make_synthetic_trace("synth", "synth.map", stale_seed, 15, 200, "");
        Ok::<_, SyntheticOracleError>((left, right))
    };

    let config_b = MinimizationConfig::new(original_seed, vec![50u64], 5);
    let result_b = minimize_differential_seed(config_b, oracle_stale_trace);

    match result_b {
        Err(MinimizationError::CandidateTracePair {
            candidate_seed,
            error: TracePairError::StaleSeed { expected_seed, .. },
        }) => {
            assert_eq!(candidate_seed, 50);
            assert_eq!(expected_seed, 50);
        }
        other => panic!("expected CandidateTracePair::StaleSeed, got {other:?}"),
    }

    let oracle_mismatched_scenario = |seed: u64| {
        let left = make_synthetic_trace("scenario-a", "synth.map", seed, 15, 100, "");
        let right_scenario = if seed == 50 {
            "scenario-b"
        } else {
            "scenario-a"
        };
        let right = make_synthetic_trace(right_scenario, "synth.map", seed, 15, 200, "");
        Ok::<_, SyntheticOracleError>((left, right))
    };

    let config_c = MinimizationConfig::new(original_seed, vec![50u64], 5);
    let result_c = minimize_differential_seed(config_c, oracle_mismatched_scenario);

    match result_c {
        Err(MinimizationError::CandidateTracePair {
            candidate_seed,
            error: TracePairError::ScenarioMismatch { left, right },
        }) => {
            assert_eq!(candidate_seed, 50);
            assert_eq!(left, "scenario-a");
            assert_eq!(right, "scenario-b");
        }
        other => panic!("expected CandidateTracePair::ScenarioMismatch, got {other:?}"),
    }
}

/// `dedup`: Duplicate seeds skipped without consuming oracle budget.
#[test]
fn dedup() {
    let original_seed = 100u64;
    let eval_count = AtomicUsize::new(0);

    let oracle = |seed: u64| {
        eval_count.fetch_add(1, Ordering::SeqCst);
        let left = make_synthetic_trace("synth", "synth.map", seed, 15, 100, "");
        let right = make_synthetic_trace("synth", "synth.map", seed, 15, 200, "");
        Ok::<_, SyntheticOracleError>((left, right))
    };

    let candidates = vec![100u64, 50, 50, 100, 25, 50, 25];
    let config = MinimizationConfig::new(original_seed, candidates, 20);
    let result = minimize_differential_seed(config, oracle).expect("minimization should succeed");

    assert_eq!(result.oracle_evaluations, 3);
    assert_eq!(eval_count.load(Ordering::SeqCst), 3);
    assert_eq!(result.minimum_seed, 25);
    assert!(result.is_complete());
}

/// `numericalmininfinitecorpus`: Infinite stream bounded by budget, no hang.
#[test]
fn numericalmininfinitecorpus() {
    let original_seed = 1000u64;

    let oracle = |seed: u64| {
        let right_x = if seed.is_multiple_of(2) { 200 } else { 100 };
        let left = make_synthetic_trace("synth", "synth.map", seed, 15, 100, "");
        let right = make_synthetic_trace("synth", "synth.map", seed, 15, right_x, "");
        Ok::<_, SyntheticOracleError>((left, right))
    };

    let infinite_candidates = 0u64..;
    let config =
        MinimizationConfig::new(original_seed, infinite_candidates, 6).with_max_inspections(20);
    let result = minimize_differential_seed(config, oracle).expect("minimization should succeed");

    assert_eq!(result.oracle_evaluations, 6);
    assert_eq!(result.minimum_seed, 0);
    assert!(result.is_oracle_budget_exhausted());
    assert!(result.reduced());
}

/// `genuinetraceoracle`: Real pcx_play runner + first_divergence on synthetic variants.
#[test]
fn genuinetraceoracle() {
    let variant_a = Scenario {
        name: "smoke-diff-100".to_string(),
        map: "synthetic.smoke-diff".to_string(),
        ticks: 20,
        spawns: vec![SpawnSpec {
            owner: 0,
            x: 0,
            y: 0,
        }],
        script: vec![ScheduledAction {
            tick: 10,
            name: "move-a".to_string(),
            action: ScriptedAction::Commands {
                player: 0,
                commands: vec![Command::Move {
                    x: FX_ONE * 10,
                    y: 0,
                }],
            },
        }],
    };

    let variant_b = Scenario {
        name: "smoke-diff-100".to_string(),
        map: "synthetic.smoke-diff".to_string(),
        ticks: 20,
        spawns: vec![SpawnSpec {
            owner: 0,
            x: 0,
            y: 0,
        }],
        script: vec![ScheduledAction {
            tick: 10,
            name: "move-b".to_string(),
            action: ScriptedAction::Commands {
                player: 0,
                commands: vec![Command::Move {
                    x: FX_ONE * 20,
                    y: 0,
                }],
            },
        }],
    };

    let original_seed = 128u64;

    let oracle = |seed: u64| {
        let mut config_a = smoke_config();
        config_a.seed = seed;
        let trace_a = pcx_play::run(&variant_a, &config_a)?;

        let mut config_b = smoke_config();
        config_b.seed = seed;
        let trace_b = pcx_play::run(&variant_b, &config_b)?;

        Ok::<_, pcx_play::RunError>((trace_a, trace_b))
    };

    let candidates: Vec<u64> = search_scope::bit_clearing(original_seed)
        .chain(search_scope::halving(original_seed))
        .collect();

    let config = MinimizationConfig::new(original_seed, candidates, 20)
        .with_predicate(FailureSignature::SameField);
    let result = minimize_differential_seed(config, oracle).expect("minimization should succeed");

    assert_eq!(result.original_seed, 128);
    assert_eq!(result.original_divergence.tick, Some(10));
    assert_eq!(result.minimum_divergence.tick, Some(10));
    assert_eq!(
        result.original_divergence.field,
        result.minimum_divergence.field
    );
    assert!(result.minimum_seed < result.original_seed);
    assert!(result.reduced());

    let variant_c = Scenario {
        name: "incompatible-scenario".to_string(),
        map: "synthetic.smoke-diff".to_string(),
        ticks: 20,
        spawns: vec![SpawnSpec {
            owner: 0,
            x: 0,
            y: 0,
        }],
        script: Vec::new(),
    };

    let incompatible_oracle = |seed: u64| {
        let mut cfg_a = smoke_config();
        cfg_a.seed = seed;
        let trace_a = pcx_play::run(&variant_a, &cfg_a)?;

        let mut cfg_c = smoke_config();
        cfg_c.seed = seed;
        let trace_c = pcx_play::run(&variant_c, &cfg_c)?;

        Ok::<_, pcx_play::RunError>((trace_a, trace_c))
    };

    let incompatible_config = MinimizationConfig::new(original_seed, vec![64u64], 5);
    let incompatible_result = minimize_differential_seed(incompatible_config, incompatible_oracle);

    match incompatible_result {
        Err(MinimizationError::OriginalSeedTracePair(TracePairError::ScenarioMismatch {
            left,
            right,
        })) => {
            assert_eq!(left, "smoke-diff-100");
            assert_eq!(right, "incompatible-scenario");
        }
        other => panic!("expected OriginalSeedTracePair ScenarioMismatch, got {other:?}"),
    }
}

/// `unsupportedversionrejected`: Unsupported trace format versions refused by pair validator.
#[test]
fn unsupportedversionrejected() {
    use pcx_play::{AssetMode, TickRecord, Trace, TraceHeader, UnitState};

    fn trace_version(seed: u64, version: u32) -> Trace {
        Trace {
            header: TraceHeader {
                format_version: version,
                scenario: "synthetic".into(),
                map: "synthetic-map".into(),
                asset_mode: AssetMode::Vanilla,
                seed,
                fix_rows: vec![],
                tick_count: 1,
            },
            ticks: vec![TickRecord {
                tick: 0,
                units: vec![UnitState {
                    id: 0,
                    owner: 0,
                    x: 0,
                    y: 0,
                    target: None,
                }],
                rng_draws: vec![],
                timers: vec![],
            }],
        }
    }

    let left = trace_version(8, 2);
    let right = trace_version(8, 2);
    // The canonical decoder also refuses unsupported versions.
    let pair_result = minimize::validate_trace_pair(8, &left, &right);
    assert!(
        pair_result.is_err(),
        "unsupported format version must be rejected by pair validator"
    );
}

/// `noncanonicalfixrowsrejected`: Noncanonical fix rows (duplicate/non-ascending/invalid ids).
#[test]
fn noncanonicalfixrowsrejected() {
    use pcx_play::{AssetMode, FixDisposition, FixRow, TickRecord, Trace, TraceHeader, UnitState};

    fn trace_fix_rows(seed: u64, fix_rows: Vec<FixRow>) -> Trace {
        Trace {
            header: TraceHeader {
                format_version: 1,
                scenario: "synthetic".into(),
                map: "synthetic-map".into(),
                asset_mode: AssetMode::Vanilla,
                seed,
                fix_rows,
                tick_count: 1,
            },
            ticks: vec![TickRecord {
                tick: 0,
                units: vec![UnitState {
                    id: 0,
                    owner: 0,
                    x: 0,
                    y: 0,
                    target: None,
                }],
                rng_draws: vec![],
                timers: vec![],
            }],
        }
    }

    // Duplicate fix row ids: non-ascending / duplicate order.
    let left_dup = trace_fix_rows(
        8,
        vec![
            FixRow {
                id: "duplicate".into(),
                disposition: FixDisposition::Defer,
            },
            FixRow {
                id: "duplicate".into(),
                disposition: FixDisposition::Defer,
            },
        ],
    );
    let mut right_dup = left_dup.clone();
    right_dup.ticks[0].units[0].x = 1; // divergence for comparison

    assert!(
        minimize::validate_trace_pair(8, &left_dup, &right_dup).is_err(),
        "duplicate fix rows must be rejected"
    );

    // Non-ascending ids.
    let left_asc = trace_fix_rows(
        8,
        vec![
            FixRow {
                id: "b".into(),
                disposition: FixDisposition::Adopt,
            },
            FixRow {
                id: "a".into(),
                disposition: FixDisposition::Reject,
            },
        ],
    );
    let right_asc = left_asc.clone();
    assert!(
        minimize::validate_trace_pair(8, &left_asc, &right_asc).is_err(),
        "non-ascending fix rows must be rejected"
    );

    // Invalid canonical name in fix row id.
    let left_bad_name = trace_fix_rows(
        8,
        vec![FixRow {
            id: "bad!name".into(),
            disposition: FixDisposition::Adopt,
        }],
    );
    assert!(
        minimize::validate_trace_pair(8, &left_bad_name, &left_bad_name.clone()).is_err(),
        "invalid canonical fix row id must be rejected"
    );
}

/// `inspectionbudgetbeforeadvancement`: Budget check precedes iterator next().
/// When the original verification already spent the complete inspection budget,
/// the iterator must not be advanced.
#[test]
fn inspectionbudgetbeforeadvancement() {
    let calls = std::cell::Cell::new(0);
    let corpus = std::iter::from_fn(|| {
        calls.set(calls.get() + 1);
        Some(7)
    });

    let config = MinimizationConfig::new(8, corpus, 2).with_max_inspections(1);
    let result = minimize_differential_seed(config, |seed: u64| {
        Ok::<_, std::io::Error>((
            make_synthetic_trace("synth", "synth.map", seed, 15, 100, ""),
            make_synthetic_trace("synth", "synth.map", seed, 15, 200, ""),
        ))
    });
    assert!(result.is_ok());
    assert_eq!(
        calls.get(),
        0,
        "original verification spent complete inspection budget; iterator must not advance"
    );
}

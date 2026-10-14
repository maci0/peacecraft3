//! Synthetic minimization gates; these checks make no native behavior claim.

use std::cell::Cell;

use pcx_play::minimize::{
    MinimizationConfig, MinimizationError, MinimizationStatus, TracePairError,
    minimize_differential_seed, validate_trace_pair,
};
use pcx_play::{AssetMode, FixDisposition, FixRow, RngDraw, TimerEvent, TimerEventKind, Trace};

fn pair(seed: u64) -> (Trace, Trace) {
    let mut config = pcx_play::smoke_config();
    config.seed = seed;
    let left = pcx_play::run(&pcx_play::smoke(), &config).unwrap();
    assert_eq!(Trace::decode(&left.encode()).unwrap(), left);
    let mut right = left.clone();
    right.ticks[0].units[0].x += 1;
    (left, right)
}

fn duplicate_units(trace: &mut Trace) {
    let unit = trace.ticks[0].units[0].clone();
    trace.ticks[0].units.insert(0, unit);
}

fn descending_units(trace: &mut Trace) {
    let mut unit = trace.ticks[0].units[0].clone();
    unit.id = u32::MAX;
    trace.ticks[0].units.insert(0, unit);
}

fn descending_rng_across_ticks(trace: &mut Trace) {
    for tick in &mut trace.ticks {
        tick.rng_draws.clear();
    }
    trace.ticks[0]
        .rng_draws
        .push(RngDraw { index: 2, value: 0 });
    trace.ticks[1]
        .rng_draws
        .push(RngDraw { index: 1, value: 0 });
}

fn descending_timers_across_ticks(trace: &mut Trace) {
    for tick in &mut trace.ticks {
        tick.timers.clear();
    }
    for (tick, seq) in [(0, 2), (1, 1)] {
        trace.ticks[tick].timers.push(TimerEvent {
            seq,
            kind: TimerEventKind::Fire,
            name: "synthetic-event".into(),
        });
    }
}

fn invalid_timer_name(trace: &mut Trace) {
    for tick in &mut trace.ticks {
        tick.timers.clear();
    }
    trace.ticks[0].timers.push(TimerEvent {
        seq: 0,
        kind: TimerEventKind::Fire,
        name: "invalid name".into(),
    });
}

#[test]
fn malformed_records_are_errors_before_failure_matching() {
    let mutations: [fn(&mut Trace); 5] = [
        duplicate_units,
        descending_units,
        descending_rng_across_ticks,
        descending_timers_across_ticks,
        invalid_timer_name,
    ];
    for mutate in mutations {
        for malformed_original in [true, false] {
            for malformed_left in [true, false] {
                let inspections = Cell::new(0);
                let corpus = std::iter::from_fn(|| {
                    inspections.set(inspections.get() + 1);
                    Some(7)
                });
                let mut calls = Vec::new();
                let result =
                    minimize_differential_seed(MinimizationConfig::new(100, corpus, 4), |seed| {
                        calls.push(seed);
                        let (mut left, mut right) = pair(seed);
                        if (seed == 100) == malformed_original {
                            let malformed = if malformed_left {
                                &mut left
                            } else {
                                &mut right
                            };
                            mutate(malformed);
                            assert!(Trace::decode(&malformed.encode()).is_err());
                        }
                        Ok::<_, std::io::Error>((left, right))
                    });
                if malformed_original {
                    assert!(matches!(
                        result,
                        Err(MinimizationError::OriginalSeedTracePair(
                            TracePairError::MalformedRecord { .. }
                        ))
                    ));
                    assert_eq!(calls, [100]);
                    assert_eq!(inspections.get(), 0);
                } else {
                    assert!(matches!(
                        result,
                        Err(MinimizationError::CandidateTracePair {
                            candidate_seed: 7,
                            error: TracePairError::MalformedRecord { .. },
                        })
                    ));
                    assert_eq!(calls, [100, 7]);
                    assert_eq!(inspections.get(), 1);
                }
            }
        }
    }
}

#[test]
fn matching_candidate_pairs_must_keep_original_identity() {
    for field in ["scenario", "map", "asset-mode", "fix-rows", "ticks"] {
        let result = minimize_differential_seed(MinimizationConfig::new(100, [7], 4), |seed| {
            let (mut left, mut right) = pair(seed);
            if seed == 7 {
                for trace in [&mut left, &mut right] {
                    match field {
                        "scenario" => trace.header.scenario = "another-scenario".into(),
                        "map" => trace.header.map = "another-map".into(),
                        "asset-mode" => trace.header.asset_mode = AssetMode::Reforged,
                        "fix-rows" => {
                            trace.header.fix_rows = vec![FixRow {
                                id: "synthetic-fix".into(),
                                disposition: FixDisposition::Adopt,
                            }]
                        }
                        "ticks" => {
                            trace.ticks.pop();
                            trace.header.tick_count -= 1;
                        }
                        _ => unreachable!(),
                    }
                    assert_eq!(Trace::decode(&trace.encode()).unwrap(), *trace);
                }
            }
            Ok::<_, std::io::Error>((left, right))
        });
        assert!(
            matches!(
                result,
                Err(MinimizationError::CandidateTracePair {
                    candidate_seed: 7,
                    ..
                })
            ),
            "changed {field} must not be accepted: {result:?}"
        );
    }
}

#[test]
fn pair_validator_rejects_fix_configuration_differences() {
    for changed_row in [
        None,
        Some(FixRow {
            id: "synthetic-fix".into(),
            disposition: FixDisposition::Reject,
        }),
        Some(FixRow {
            id: "another-fix".into(),
            disposition: FixDisposition::Adopt,
        }),
    ] {
        let (mut left, mut right) = pair(100);
        left.header.fix_rows = vec![FixRow {
            id: "synthetic-fix".into(),
            disposition: FixDisposition::Adopt,
        }];
        right.header.fix_rows = changed_row.into_iter().collect();
        assert!(matches!(validate_trace_pair(100, &left, &right),
            Err(TracePairError::HeaderFieldMismatch { field, .. }) if field == "fix-rows"
        ));
    }
}

#[test]
fn zero_budgets_invoke_neither_oracle_nor_iterator() {
    for (oracle_budget, inspection_budget) in [(0, 4), (4, 0)] {
        let corpus = std::iter::from_fn(|| -> Option<u64> { panic!("iterator advanced") });
        let result = minimize_differential_seed(
            MinimizationConfig::new(100, corpus, oracle_budget)
                .with_max_inspections(inspection_budget),
            |_| -> Result<(Trace, Trace), std::io::Error> { panic!("oracle invoked") },
        );
        if oracle_budget == 0 {
            assert!(matches!(
                result,
                Err(MinimizationError::ZeroOracleEvaluationBudget)
            ));
        } else {
            assert!(matches!(
                result,
                Err(MinimizationError::ZeroInspectionBudget)
            ));
        }
    }
}

#[test]
fn exhausted_oracle_budget_does_not_advance_iterator() {
    for inspections in [1, 10] {
        let corpus = std::iter::from_fn(|| -> Option<u64> { panic!("iterator advanced") });
        let result = minimize_differential_seed(
            MinimizationConfig::new(100, corpus, 1).with_max_inspections(inspections),
            |seed| Ok::<_, std::io::Error>(pair(seed)),
        )
        .unwrap();
        assert_eq!(result.oracle_evaluations, 1);
        assert_eq!(result.inspections, 1);
        assert_eq!(
            result.status,
            if inspections == 1 {
                MinimizationStatus::InspectionBudgetExhausted
            } else {
                MinimizationStatus::BudgetExhausted
            }
        );
    }
}

#[test]
fn infinite_duplicates_are_bounded_without_reexecution() {
    let inspected = Cell::new(0);
    let corpus = std::iter::from_fn(|| {
        inspected.set(inspected.get() + 1);
        Some(100)
    });
    let mut calls = Vec::new();
    let result = minimize_differential_seed(
        MinimizationConfig::new(100, corpus, 10).with_max_inspections(4),
        |seed| {
            calls.push(seed);
            Ok::<_, std::io::Error>(pair(seed))
        },
    )
    .unwrap();
    assert_eq!(calls, [100]);
    assert_eq!(inspected.get(), 3);
    assert_eq!(result.inspections, 4);
    assert_eq!(result.oracle_evaluations, 1);
    assert_eq!(result.status, MinimizationStatus::InspectionBudgetExhausted);
}

#[test]
fn unsorted_nonmonotone_corpus_returns_smallest_verified_reproduction() {
    let mut calls = Vec::new();
    let result = minimize_differential_seed(
        MinimizationConfig::new(100, [90, 5, 20, 7, 3, 90, 100], 20),
        |seed| {
            calls.push(seed);
            let (left, mut right) = pair(seed);
            if matches!(seed, 3 | 5) {
                right = left.clone();
            }
            Ok::<_, std::io::Error>((left, right))
        },
    )
    .unwrap();
    assert_eq!(calls, [100, 90, 5, 20, 7, 3]);
    assert_eq!(result.minimum_seed, 7);
    assert_eq!(result.oracle_evaluations, 6);
    assert_eq!(result.inspections, 8);
    assert_eq!(result.status, MinimizationStatus::CorpusExhausted);
    assert_eq!(result.minimum_divergence, result.original_divergence);
}

//! Seeded movement-profile tests; no original-runtime oracle is implied.

use pcx_play::generator::{ActionKind, Error, Limits, MovementSpec, generate};
use pcx_play::{AssetMode, FixDisposition, FixRow, Rpl, RunConfig, ScriptedAction};

const KINDS: &[ActionKind] = &[
    ActionKind::Move,
    ActionKind::Select,
    ActionKind::Stop,
    ActionKind::Nop,
    ActionKind::RngDraw,
];

fn spec() -> MovementSpec<'static> {
    MovementSpec {
        scenario: "fuzz-v1",
        map: "synthetic.fuzz-v1",
        ticks: 7,
        units: 3,
        actions: 12,
        owners: &[0, 2, 255],
        x: -70_000..=90_000,
        y: -3..=8,
        action_kinds: KINDS,
    }
}

fn config(seed: u64) -> RunConfig {
    RunConfig {
        asset_mode: AssetMode::Hd,
        seed,
        fix_rows: vec![FixRow {
            id: "synthetic-row".into(),
            disposition: FixDisposition::Defer,
        }],
    }
}

fn limits() -> Limits {
    Limits {
        max_units: 100,
        max_actions: 100,
        max_ticks: 100,
        max_fix_rows: 2,
        max_work: 10_000,
        max_output_bytes: 16_384,
    }
}

#[test]
fn generated_domains_and_identity_survive_strict_ingestion() {
    for seed in [0, 1, u64::MAX] {
        let spec = spec();
        let config = config(seed);
        let source = generate(&spec, &config, limits()).unwrap();
        let parsed = Rpl::parse(&source).unwrap();
        assert_eq!(parsed.config, config);
        assert_eq!(parsed.scenario.name, spec.scenario);
        assert_eq!(parsed.scenario.map, spec.map);
        assert_eq!(parsed.scenario.spawns.len(), spec.units);
        assert_eq!(parsed.scenario.script.len(), spec.actions);
        for spawn in &parsed.scenario.spawns {
            assert!(spec.owners.contains(&spawn.owner));
            assert!(spec.x.contains(&spawn.x) && spec.y.contains(&spawn.y));
        }
        for entry in &parsed.scenario.script {
            assert!(entry.tick < spec.ticks);
            if let ScriptedAction::Commands { player, commands } = &entry.action {
                assert!(spec.owners.contains(player));
                for command in commands {
                    if let pcx_sim::Command::Move { x, y } | pcx_sim::Command::Select { x, y } =
                        command
                    {
                        assert!(spec.x.contains(x) && spec.y.contains(y));
                    }
                }
            }
        }
        let trace = parsed.run().unwrap();
        assert_eq!(pcx_play::Trace::decode(&trace.encode()).unwrap(), trace);
    }
}

#[test]
fn generation_and_one_hundred_fresh_runs_are_byte_identical() {
    let source = generate(&spec(), &config(9821), limits()).unwrap();
    let parsed = Rpl::parse(&source).unwrap();
    let first = parsed.run().unwrap().encode();
    for _ in 0..100 {
        assert_eq!(generate(&spec(), &config(9821), limits()).unwrap(), source);
        assert_eq!(parsed.run().unwrap().encode(), first);
    }
    assert_ne!(generate(&spec(), &config(9822), limits()).unwrap(), source);
}

#[test]
fn full_signed_coordinate_domain_and_singleton_ranges_do_not_overflow() {
    let mut full = spec();
    full.x = i32::MIN..=i32::MAX;
    full.y = i32::MAX..=i32::MAX;
    full.action_kinds = &[ActionKind::Move];
    let source = generate(&full, &config(u64::MAX), limits()).unwrap();
    let parsed = Rpl::parse(&source).unwrap();
    assert!(
        parsed
            .scenario
            .spawns
            .iter()
            .all(|spawn| spawn.y == i32::MAX)
    );
    parsed.run().unwrap();
}

#[test]
fn all_supported_action_kinds_and_zero_action_run_are_real_rpl() {
    for kind in KINDS {
        let mut spec = spec();
        spec.ticks = 1;
        spec.action_kinds = std::slice::from_ref(kind);
        let source = generate(&spec, &config(17), limits()).unwrap();
        let parsed = Rpl::parse(&source).unwrap();
        let trace = parsed.run().unwrap();
        assert_eq!(trace.ticks[0].timers.len(), spec.actions);
        for (index, timer) in trace.ticks[0].timers.iter().enumerate() {
            assert_eq!(timer.name, format!("generated-{index}"));
        }
        assert_eq!(
            trace.ticks[0].rng_draws.len(),
            if *kind == ActionKind::RngDraw {
                spec.actions
            } else {
                0
            }
        );
    }
    let mut empty = spec();
    empty.units = 0;
    empty.actions = 0;
    empty.action_kinds = &[];
    Rpl::parse(&generate(&empty, &config(1), limits()).unwrap())
        .unwrap()
        .run()
        .unwrap();
}

#[test]
fn invalid_domains_and_configurations_are_refused_before_generation() {
    let mut invalids = Vec::new();
    let mut bad = spec();
    bad.owners = &[];
    invalids.push(bad);
    let mut bad = spec();
    bad.owners = &[0, 0];
    invalids.push(bad);
    let mut bad = spec();
    bad.action_kinds = &[];
    invalids.push(bad);
    let mut bad = spec();
    bad.action_kinds = &[ActionKind::Move, ActionKind::Move];
    invalids.push(bad);
    let mut bad = spec();
    bad.x = std::ops::RangeInclusive::new(8, -2);
    invalids.push(bad);
    let mut bad = spec();
    bad.ticks = 0;
    invalids.push(bad);
    let mut bad = spec();
    bad.scenario = "bad\nrecord";
    invalids.push(bad);
    let mut bad = spec();
    bad.map = "";
    invalids.push(bad);
    for bad in invalids {
        assert!(matches!(
            generate(&bad, &config(1), limits()),
            Err(Error::Invalid { .. })
        ));
    }
    let mut duplicate = config(1);
    duplicate.fix_rows.push(duplicate.fix_rows[0].clone());
    assert!(matches!(
        generate(&spec(), &duplicate, limits()),
        Err(Error::Invalid {
            field: "fix rows",
            ..
        })
    ));
}

#[test]
fn exact_output_work_and_cardinality_budgets_are_enforced() {
    let source = generate(&spec(), &config(1), limits()).unwrap();
    let exact = Limits {
        max_units: 3,
        max_actions: 12,
        max_ticks: 7,
        max_fix_rows: 1,
        max_work: 40,
        max_output_bytes: source.len(),
    };
    assert_eq!(generate(&spec(), &config(1), exact).unwrap(), source);
    for low in [
        Limits {
            max_units: 2,
            ..exact
        },
        Limits {
            max_actions: 11,
            ..exact
        },
        Limits {
            max_ticks: 6,
            ..exact
        },
        Limits {
            max_fix_rows: 0,
            ..exact
        },
        Limits {
            max_work: 39,
            ..exact
        },
        Limits {
            max_output_bytes: source.len() - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            generate(&spec(), &config(1), low),
            Err(Error::Limit { .. })
        ));
    }
    let mut huge = spec();
    huge.ticks = u64::MAX;
    let allowed = Limits {
        max_ticks: u64::MAX,
        max_work: u64::MAX,
        ..limits()
    };
    assert!(matches!(
        generate(&huge, &config(1), allowed),
        Err(Error::Layout { field: "ticks" })
    ));
}

#[test]
fn fresh_generated_pair_runs_integrate_with_seed_minimization() {
    use pcx_play::minimize::{MinimizationConfig, minimize_differential_seed};
    let left_spec = MovementSpec {
        ticks: 2,
        units: 1,
        actions: 1,
        x: 0..=0,
        y: 0..=0,
        action_kinds: &[ActionKind::Move],
        ..spec()
    };
    let right_spec = MovementSpec {
        x: 1..=1,
        ..left_spec.clone()
    };
    let result = minimize_differential_seed(
        MinimizationConfig::new(99, [3, 0, 7], 4).with_max_inspections(5),
        |seed| {
            let left = Rpl::parse(&generate(&left_spec, &config(seed), limits()).unwrap())
                .unwrap()
                .run()?;
            let right = Rpl::parse(&generate(&right_spec, &config(seed), limits()).unwrap())
                .unwrap()
                .run()?;
            Ok::<_, String>((left, right))
        },
    )
    .unwrap();
    assert_eq!(result.minimum_seed, 0);
    assert_eq!(result.oracle_evaluations, 4);
    assert_eq!(result.minimum_divergence.field, "unit[0].x");
}

#[test]
fn cli_emits_executable_rpl_and_refusals_do_not_emit_partial_output() {
    use std::process::Command;
    let args = [
        "33", "4", "2", "9", "-20", "30", "0", "7", "21", "4096", "0", "255",
    ];
    let call = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_playgen"))
            .args(args)
            .output()
            .unwrap()
    };
    let first = call(&args);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let parsed = Rpl::parse(std::str::from_utf8(&first.stdout).unwrap()).unwrap();
    parsed.run().unwrap();
    assert_eq!(call(&args).stdout, first.stdout);
    let mut bad = args;
    bad[8] = "20";
    let refused = call(&bad);
    assert_eq!(refused.status.code(), Some(2));
    assert!(refused.stdout.is_empty());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("work requires 21"));
    bad = args;
    bad[9] = "1";
    let refused = call(&bad);
    assert_eq!(refused.status.code(), Some(2));
    assert!(refused.stdout.is_empty());
}

#[test]
fn checked_in_seed_corpus_regenerates_and_repeats_without_drift() {
    for (seed, source) in [
        (
            0,
            include_str!("../../../tests/play/synthetic/generated-movement/seed-0.rpl"),
        ),
        (
            1,
            include_str!("../../../tests/play/synthetic/generated-movement/seed-1.rpl"),
        ),
        (
            u64::MAX,
            include_str!("../../../tests/play/synthetic/generated-movement/seed-max.rpl"),
        ),
    ] {
        let spec = MovementSpec {
            scenario: "generated-movement-v1",
            map: "synthetic.generated-movement-v1",
            ticks: 12,
            units: 8,
            actions: 24,
            owners: &[0, 1],
            x: i32::MIN..=i32::MAX,
            y: -100_000..=100_000,
            action_kinds: KINDS,
        };
        let config = RunConfig {
            asset_mode: AssetMode::Vanilla,
            seed,
            fix_rows: Vec::new(),
        };
        assert_eq!(generate(&spec, &config, limits()).unwrap(), source);
        let parsed = Rpl::parse(source).unwrap();
        let expected = parsed.run().unwrap().encode();
        for _ in 0..100 {
            assert_eq!(parsed.run().unwrap().encode(), expected);
        }
    }
}

//! `playgen` CLI contract: argument mapping, refusal behaviour, and one
//! seed's byte-identical output.
//!
//! These are harness checks over the shipped binary. `playgen` emits the
//! synthetic movement-v1 recipe (see `docs/playtest-generation.md`); nothing
//! here is a claim about the original client, and the generated RPL is not
//! original-game evidence.
//!
//! The generator's own integration test (`tests/generator.rs`) covers the
//! library path and the checked-in seed corpus. This file owns the command's
//! contract: which argument reaches which field, what a refusal looks like on
//! each stream, and what repeats byte for byte.

use std::process::Command;

use pcx_play::{AssetMode, Rpl, ScriptedAction, fnv1a64};
use pcx_sim::Command as SimCommand;

/// Runs the shipped `playgen` binary with `args`.
fn playgen(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_playgen"))
        .args(args)
        .output()
        .unwrap()
}

/// The CLI's documented argument order:
/// `SEED TICKS UNITS ACTIONS MIN_X MAX_X MIN_Y MAX_Y MAX_WORK MAX_BYTES OWNER...`.
fn base_args() -> Vec<&'static str> {
    vec![
        "42", "100", "4", "8", "-20", "20", "-20", "20", "32768", "16384", "0",
    ]
}

/// The same shape with disjoint x and y windows, so an argument that lands in
/// the wrong field cannot pass unnoticed.
fn offset_args() -> Vec<&'static str> {
    vec![
        "42", "100", "4", "8", "-20", "-10", "5", "9", "32768", "16384", "0",
    ]
}

fn stderr_of(result: &std::process::Output) -> String {
    String::from_utf8_lossy(&result.stderr).into_owned()
}

#[test]
fn missing_args_report_usage_on_stderr_with_silent_stdout() {
    let result = playgen(&[]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty(), "errors must not pollute stdout");
    assert!(
        stderr_of(&result).contains("usage:"),
        "{}",
        stderr_of(&result)
    );
}

#[test]
fn invalid_arguments_are_refused_on_stderr_with_silent_stdout() {
    // Each case names the bound or field it broke, so a refusal stays
    // diagnosable instead of collapsing into a generic failure.
    let cases: [(&[&str], &str); 4] = [
        (
            &[
                "abc", "100", "4", "8", "-20", "20", "-20", "20", "32768", "16384", "0",
            ],
            "invalid seed",
        ),
        (
            &[
                "42", "100", "4", "8", "20", "-20", "-20", "20", "32768", "16384", "0",
            ],
            "reversed inclusive range",
        ),
        (
            &[
                "42", "100", "4", "8", "-20", "20", "-20", "20", "1", "16384", "0",
            ],
            "requires",
        ),
        (
            &[
                "42", "100", "4", "8", "-20", "20", "-20", "20", "32768", "16384", "256",
            ],
            "invalid owner",
        ),
    ];
    for (args, expected) in cases {
        let result = playgen(args);
        assert_eq!(result.status.code(), Some(2), "args={args:?}");
        assert!(result.stdout.is_empty(), "args={args:?}");
        let stderr = stderr_of(&result);
        assert!(
            !stderr.is_empty(),
            "a diagnostic is required: args={args:?}"
        );
        assert!(
            stderr.contains(expected),
            "args={args:?} stderr={stderr:?} must name {expected:?}"
        );
    }
}

#[test]
fn the_work_budget_refusal_names_the_work_budget() {
    // The tenth argument is the work budget, so a request it cannot afford
    // is refused in those words. The number it names is the request's own
    // work, counted by the shared model.
    let mut args = base_args();
    *args.get_mut(8).unwrap() = "1";
    let result = playgen(&args);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty(), "a refused run emits no scenario");
    let stderr = stderr_of(&result);
    // 100 ticks, 4 units and 8 actions cost 100 * (4 + 1) + 8 = 508.
    assert!(stderr.contains("work requires 508"), "{stderr}");
    assert!(
        !stderr.contains("ticks requires"),
        "the budget is a work budget, not a tick limit: {stderr}"
    );
}

#[test]
fn the_output_ceiling_refuses_instead_of_truncating() {
    // 16 bytes cannot hold the header, so the run is refused rather than
    // emitting a truncated scenario that would still parse.
    let mut args = base_args();
    *args.get_mut(9).unwrap() = "16";
    let result = playgen(&args);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty(), "a refused run emits no scenario");
    assert!(
        stderr_of(&result).contains("output bytes"),
        "{}",
        stderr_of(&result)
    );
}

#[test]
fn the_same_seed_emits_byte_identical_rpl_and_trace() {
    let first = playgen(&base_args());
    assert!(first.status.success());
    assert!(
        first.stderr.is_empty(),
        "a successful run has no diagnostic"
    );
    let second = playgen(&base_args());
    assert_eq!(first.stdout, second.stdout);
    let source = String::from_utf8(first.stdout).unwrap();
    let parsed = Rpl::parse(&source).unwrap();
    let expected = parsed.run().unwrap().encode();
    assert_eq!(parsed.run().unwrap().encode(), expected);
    // The same scenario run from the second process carries the same trace.
    let other = Rpl::parse(&String::from_utf8(second.stdout).unwrap()).unwrap();
    assert_eq!(other.run().unwrap().encode(), expected);
}

#[test]
fn a_different_seed_emits_different_bytes() {
    // The byte-identical check above is only meaningful if the seed reaches
    // the generator; this proves it is load-bearing.
    let mut other = base_args();
    *other.get_mut(0).unwrap() = "43";
    let first = playgen(&base_args());
    let second = playgen(&other);
    assert!(first.status.success() && second.status.success());
    // Hashes keep a failure readable; the raw bytes are far too long to print.
    assert_ne!(
        fnv1a64(&first.stdout),
        fnv1a64(&second.stdout),
        "the seed must reach the generator"
    );
}

#[test]
fn the_arguments_map_onto_the_generated_scenario() {
    let result = playgen(&offset_args());
    assert!(result.status.success(), "{}", stderr_of(&result));
    let source = String::from_utf8(result.stdout).unwrap();
    let parsed = Rpl::parse(&source).unwrap();

    assert_eq!(parsed.config.seed, 42, "seed argument");
    assert_eq!(parsed.scenario.ticks, 100, "ticks argument");
    assert_eq!(parsed.scenario.spawns.len(), 4, "units argument");
    assert_eq!(parsed.scenario.script.len(), 8, "actions argument");
    assert_eq!(parsed.config.asset_mode, AssetMode::Vanilla);

    for spawn in &parsed.scenario.spawns {
        assert_eq!(spawn.owner, 0, "owner argument");
        assert!((-20..=-10).contains(&spawn.x), "x window: {}", spawn.x);
        assert!((5..=9).contains(&spawn.y), "y window: {}", spawn.y);
    }
    for entry in &parsed.scenario.script {
        assert!(entry.tick < parsed.scenario.ticks, "tick {}", entry.tick);
        let ScriptedAction::Commands { player, commands } = &entry.action else {
            continue;
        };
        assert_eq!(*player, 0, "owner argument");
        for command in commands {
            let (x, y) = match command {
                SimCommand::Move { x, y } | SimCommand::Select { x, y } => (*x, *y),
                SimCommand::Stop | SimCommand::Nop => continue,
            };
            assert!((-20..=-10).contains(&x), "x window: {x}");
            assert!((5..=9).contains(&y), "y window: {y}");
        }
    }
}

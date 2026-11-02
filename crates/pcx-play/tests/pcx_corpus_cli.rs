//! The `pcx-corpus` binary, driven end to end: real recordings in, real
//! reports out.
//!
//! The fixed Tier C corpus used to be reachable only from the library's own
//! tests, so no two builds could be compared over it. These checks drive the
//! command that makes it comparable, and they check the refusal paths too.
//!
//! Harness policy over synthetic inputs. Nothing here is a claim about the
//! original client: a divergence between two recordings says that two builds
//! differ over this harness's inputs.

use std::path::{Path, PathBuf};
use std::process::Command;

use pcx_play::fuzz_corpus::corpus_entries;

/// A temporary directory that is removed when the test ends.
fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&path);
    path
}

fn corpus(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pcx-corpus"))
        .args(args)
        .output()
        .expect("pcx-corpus must run")
}

fn stdout_of(result: &std::process::Output) -> String {
    String::from_utf8_lossy(&result.stdout).into_owned()
}

fn stderr_of(result: &std::process::Output) -> String {
    String::from_utf8_lossy(&result.stderr).into_owned()
}

/// The trace file of one corpus entry, named the way the command names it.
fn trace_path(directory: &Path, seed: u64, index: usize) -> PathBuf {
    directory.join(format!("s{seed}-{index}.trace"))
}

#[test]
fn record_writes_every_corpus_entry_and_two_recordings_agree() {
    let left = scratch("pcx-corpus-left");
    let right = scratch("pcx-corpus-right");
    let left_arg = left.to_str().unwrap();
    let right_arg = right.to_str().unwrap();

    let first = corpus(&["record", left_arg]);
    assert!(first.status.success(), "{}", stderr_of(&first));
    let report = stdout_of(&first);
    let entries = corpus_entries();
    assert_eq!(entries.len(), 32);
    for (index, entry) in entries.iter().enumerate() {
        let line = format!("entry={index} seed={} hash=", entry.seed);
        assert!(
            report.contains(&line),
            "entry {index} must be reported: {report}"
        );
        // Every entry has both files, and the recorded RPL still parses and
        // produces the recorded trace.
        let rpl = left.join(format!("s{}-{index}.rpl", entry.seed));
        let trace = trace_path(&left, entry.seed, index);
        assert!(rpl.is_file(), "{}", rpl.display());
        assert!(trace.is_file(), "{}", trace.display());
        let source = std::fs::read_to_string(&rpl).unwrap();
        let parsed = pcx_play::Rpl::parse(&source).unwrap();
        assert_eq!(
            parsed.run().unwrap().encode(),
            std::fs::read(&trace).unwrap()
        );
    }

    // A second recording of the same build is byte-identical: the corpus is
    // the recipe's own output, so the recording is reproducible.
    let second = corpus(&["record", right_arg]);
    assert!(second.status.success(), "{}", stderr_of(&second));
    for (index, entry) in entries.iter().enumerate() {
        let name = format!("s{}-{index}.trace", entry.seed);
        assert_eq!(
            std::fs::read(left.join(&name)).unwrap(),
            std::fs::read(right.join(&name)).unwrap(),
            "entry {index} must record identically twice"
        );
    }

    // The comparison of two identical recordings reports every entry
    // unchanged and writes an empty filing.
    let filings = scratch("pcx-corpus-filings-empty");
    let filings_arg = filings.to_str().unwrap();
    let compared = corpus(&["compare", left_arg, right_arg, filings_arg]);
    assert!(compared.status.success(), "{}", stderr_of(&compared));
    let report = stdout_of(&compared);
    assert_eq!(
        report
            .lines()
            .filter(|line| line.contains(" unchanged"))
            .count(),
        entries.len()
    );
    assert!(report.contains("entries=32 diverged=0"), "{report}");
    assert!(compared.stderr.is_empty(), "{}", stderr_of(&compared));
    assert_eq!(std::fs::read(&filings).unwrap(), b"");
}

#[test]
fn a_divergence_is_reported_and_filed_for_the_tracker() {
    let left = scratch("pcx-corpus-diverge-left");
    let right = scratch("pcx-corpus-diverge-right");
    let left_arg = left.to_str().unwrap();
    let right_arg = right.to_str().unwrap();
    assert!(
        corpus(&["record", left_arg]).status.success(),
        "the recording must succeed"
    );
    assert!(
        corpus(&["record", right_arg]).status.success(),
        "the recording must succeed"
    );

    // The candidate build's first entry differs: its trace names another
    // seed, so the pair diverges in the header.
    let entry = &corpus_entries()[0];
    let path = trace_path(&right, entry.seed, 0);
    let mut trace = pcx_play::Trace::decode(&std::fs::read(&path).unwrap()).unwrap();
    trace.header.seed += 1;
    std::fs::write(&path, trace.encode()).unwrap();

    let filings = scratch("pcx-corpus-filings");
    let filings_arg = filings.to_str().unwrap();
    let compared = corpus(&["compare", left_arg, right_arg, filings_arg]);
    assert_eq!(compared.status.code(), Some(1));
    assert!(compared.stderr.is_empty(), "{}", stderr_of(&compared));
    let report = stdout_of(&compared);
    assert!(
        report.contains(&format!("entry=0 seed={} diverge", entry.seed)),
        "{report}"
    );
    assert!(report.contains("field=seed"), "{report}");
    assert!(report.contains("entries=32 diverged=1"), "{report}");
    assert_eq!(
        report
            .lines()
            .filter(|line| line.contains(" unchanged"))
            .count(),
        corpus_entries().len() - 1
    );

    // The filing names the reproducer: the seed, the corpus index, the
    // recipe, both trace hashes and the artifact the full RPL belongs in.
    let filing = std::fs::read_to_string(&filings).unwrap();
    assert!(filing.contains(&format!("seed {}", entry.seed)), "{filing}");
    assert!(filing.contains("corpus index 0"), "{filing}");
    assert!(filing.contains("recipe 1"), "{filing}");
    assert!(filing.contains("field=seed"), "{filing}");
    let candidate_hash = format!("{:016x}", trace.canonical_hash());
    assert!(filing.contains(&candidate_hash), "{filing}");
    assert!(
        filing.contains(&format!("diverge-fuzz-s{}-0-header.rpl", entry.seed)),
        "{filing}"
    );
    // The quoted RPL is the baseline's, so a reader reproduces the entry.
    assert!(
        filing.contains(&format!("scenario fuzz.s{}.0", entry.seed)),
        "{filing}"
    );
}

#[test]
fn the_command_refuses_a_bad_invocation_without_writing_a_report() {
    let absent = scratch("pcx-corpus-absent");
    let present = scratch("pcx-corpus-present");
    let absent_arg = absent.to_str().unwrap();
    let present_arg = present.to_str().unwrap();
    let filings = scratch("pcx-corpus-filings-refusal");
    let filings_arg = filings.to_str().unwrap();
    assert!(corpus(&["record", present_arg]).status.success());

    // Arity and unknown subcommands are usage refusals.
    for args in [
        vec![],
        vec!["bogus"],
        vec!["record"],
        vec!["compare", present_arg],
        vec!["compare", present_arg, present_arg],
        vec!["compare", present_arg, present_arg, filings_arg, "extra"],
    ] {
        let result = corpus(&args);
        assert_eq!(result.status.code(), Some(2), "args={args:?}");
        assert!(result.stdout.is_empty(), "args={args:?}");
        assert!(
            stderr_of(&result).contains("usage:"),
            "args={args:?} stderr={:?}",
            stderr_of(&result)
        );
    }

    // A recording that is not there is a refusal, not a panic, and it names
    // the file it could not read.
    let result = corpus(&["compare", absent_arg, present_arg, filings_arg]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    let stderr = stderr_of(&result);
    assert!(stderr.contains("No such file"), "{stderr}");
    assert!(!filings.exists(), "a refused comparison writes no filing");

    // A recording missing its scenario is refused too, and refused before
    // any entry is reported: the filing quotes that scenario.
    let incomplete = scratch("pcx-corpus-incomplete");
    let incomplete_arg = incomplete.to_str().unwrap();
    assert!(
        corpus(&["record", incomplete_arg]).status.success(),
        "the recording must succeed"
    );
    let entry = &corpus_entries()[0];
    std::fs::remove_file(incomplete.join(format!("s{}-0.rpl", entry.seed))).unwrap();
    let result = corpus(&["compare", incomplete_arg, present_arg, filings_arg]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty(), "no entry is reported");
    assert!(
        stderr_of(&result).contains("No such file"),
        "{}",
        stderr_of(&result)
    );
}

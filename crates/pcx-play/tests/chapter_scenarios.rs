//! The checked-in chapter scenarios must run and repeat.
//!
//! `tests/play/<campaign>/` holds one scenario per campaign chapter. They are
//! the harness's own synthetic probes — a unit, an RNG draw and a timer in the
//! simulation, with the chapter's map identity recorded in the trace header.
//! They are not campaign playthroughs, they assert no original-game outcome,
//! and they carry no shipped-byte, native-code or runtime evidence.
//!
//! The scenarios were checked in without ever being executed, so nothing
//! noticed that their records did not match the RPL grammar and that their
//! map identities were not canonical names. These tests keep them live.

use std::path::{Path, PathBuf};

use pcx_play::Rpl;

/// The checked-in chapter scenarios: the path below `tests/play` and the
/// scenario name the file must declare.
const CHAPTERS: &[(&str, &str)] = &[
    ("demo/DemoCampaign.rpl", "demo-campaign"),
    ("nightelf/chapter01.rpl", "nightelf-chapter01"),
    ("orc/chapter01.rpl", "orc-chapter01"),
    ("orc/chapter02.rpl", "orc-chapter02"),
    ("orc/chapter03.rpl", "orc-chapter03"),
    ("roc/chapter01.rpl", "roc-chapter01"),
    ("roc/chapter02.rpl", "roc-chapter02"),
    ("roc/chapter03.rpl", "roc-chapter03"),
    ("roc/chapter04.rpl", "roc-chapter04"),
    ("roc/chapter05.rpl", "roc-chapter05"),
    ("roc/chapter06.rpl", "roc-chapter06"),
    ("roc/chapter07.rpl", "roc-chapter07"),
    ("roc/chapter08.rpl", "roc-chapter08"),
    ("undead/chapter01.rpl", "undead-chapter01"),
];

/// The campaign directories that hold chapter scenarios. `synthetic` is
/// excluded: those are the generator's own fixtures.
const CAMPAIGNS: &[&str] = &["demo", "nightelf", "orc", "roc", "undead"];

fn play_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/play")
}

/// Every checked-in chapter scenario path, relative to `tests/play`, sorted.
fn chapter_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for campaign in CAMPAIGNS {
        let directory = root.join(campaign);
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{}: {error}", directory.display()));
        for entry in entries {
            let name = entry
                .expect("directory entry must read")
                .file_name()
                .to_string_lossy()
                .into_owned();
            if name.ends_with(".rpl") {
                out.push(format!("{campaign}/{name}"));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn every_checked_in_chapter_scenario_is_listed() {
    let expected: Vec<String> = CHAPTERS
        .iter()
        .map(|(path, _)| (*path).to_string())
        .collect();
    assert_eq!(
        chapter_files(&play_root()),
        expected,
        "a chapter scenario added to tests/play must be listed and run here"
    );
}

#[test]
fn every_chapter_scenario_meets_its_expectations_and_repeats_byte_identically() {
    for (relative, name) in CHAPTERS {
        let path = play_root().join(relative);
        let source =
            std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{relative}: {error}"));
        let rpl = Rpl::parse(&source).unwrap_or_else(|error| panic!("{relative}: {error}"));
        assert_eq!(rpl.scenario.name, *name, "{relative}");
        // `run` checks the scenario's own expectations, so this fails when an
        // expectation stops holding rather than only when the run crashes.
        let expected = rpl
            .run()
            .unwrap_or_else(|error| panic!("{relative}: {error}"))
            .encode();
        for run in 1..100 {
            let actual = rpl
                .run()
                .unwrap_or_else(|error| panic!("{relative} run {run}: {error}"))
                .encode();
            assert_eq!(actual, expected, "{relative} run {run}");
        }
    }
}

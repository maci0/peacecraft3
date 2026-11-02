//! Tier C fixed seed corpus (PLAN §3.5, Tier C).
//!
//! Harness policy, not game behavior. The seeds below are chosen by the
//! harness so `cargo test --workspace --features play` (the future Tier C
//! gate) runs the same scenarios everywhere. They are not the original
//! game's RNG, unit composition or terrain, and changing them does not
//! change any claim about the original client.
//!
//! Coordination: scenario parsing (`rpl`), execution (`runner`, `scenario`),
//! comparison (`trace`, `diff`) and seed minimization
//! (`tests/minimize.rs`) stay with the `mechanics` lane. This module only
//! names the corpus; generation lives in [`crate::fuzz_scenario`], resume in
//! [`crate::fuzz_resume`], and TRACKER-ready filing in
//! [`crate::fuzz_report`]. It reads those lanes' public types and never
//! edits their files.
//!
//! Bump [`CORPUS_RECIPE_VERSION`] whenever a domain, bound or selection
//! algorithm changes, so a corpus built under one recipe is never confused
//! with another.

/// Recipe version for every corpus entry in this module.
///
/// Generation and execution each start a separate `pcx_sim::Rng` at the
/// entry seed; selection is reproducible modulo indexing (see
/// `crates/pcx-play/src/generator.rs`). A domain or algorithm change
/// requires a new version.
pub const CORPUS_RECIPE_VERSION: u32 = 1;

/// Fixed seeds. Harness-chosen constants; the original client never sees
/// them. Thirty-two entries keep the default gate bounded while covering
/// small seeds, large seeds and bit-pattern edges.
pub const SEED_CORPUS: [u64; 32] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_0007,
    0x0000_0000_0000_0021,
    0x0000_0000_0000_0100,
    0x0000_0000_0001_0001,
    0x0000_0000_1234_5678,
    0x0000_0001_0000_0001,
    0x0000_00FF_0000_00FF,
    0x00FF_00FF_00FF_00FF,
    0x0F0F_0F0F_0F0F_0F0F,
    0x1111_1111_1111_1111,
    0x2222_2222_2222_2222,
    0x3333_3333_3333_3333,
    0x4444_4444_4444_4444,
    0x5555_5555_5555_5555,
    0x6666_6666_6666_6666,
    0x7777_7777_7777_7777,
    0x8888_8888_8888_8888,
    0x9999_9999_9999_9999,
    0xAAAA_AAAA_AAAA_AAAA,
    0xBBBB_BBBB_BBBB_BBBB,
    0xCCCC_CCCC_CCCC_CCCC,
    0xDDDD_DDDD_DDDD_DDDD,
    0xEEEE_EEEE_EEEE_EEEE,
    0xFFFF_FFFF_FFFF_FFFF,
    0xFFFF_FFFF_FFFF_FFFE,
    0x8000_0000_0000_0000,
    0x7FFF_FFFF_FFFF_FFFF,
    0x9E37_79B9_7F4A_7C15,
    0xBF58_476D_1CE4_E5B9,
    0x94D0_49BB_1331_11EB,
    0xDEAD_BEEF_CAFE_F00D,
];

/// One corpus entry: a seed plus a bounded shape.
///
/// Shapes stay inside [`crate::fuzz_scenario::default_limits`]: ticks is at
/// most 600, units at most 12, actions at most 24. Work per entry
/// (`ticks * (units + 1) + actions`) is at most ~7,824, so the whole
/// 32-entry corpus stays well under typical CI collection bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusEntry {
    /// Recipe version in force when this entry was defined.
    pub recipe: u32,
    /// Seed for both generation and execution.
    pub seed: u64,
    /// Tick count of the generated scenario.
    pub ticks: u64,
    /// Spawn count of the generated scenario.
    pub units: usize,
    /// Scheduled-action count of the generated scenario.
    pub actions: usize,
}

/// The fixed corpus: one entry per seed in [`SEED_CORPUS`].
///
/// Shapes cycle deterministically over three bounded sizes so the corpus
/// covers short, medium and slightly longer runs without any runtime
/// randomness:
///
/// - index % 3 == 0: 100 ticks, 4 units, 8 actions (smoke-adjacent);
/// - index % 3 == 1: 300 ticks, 8 units, 16 actions;
/// - index % 3 == 2: 600 ticks, 12 units, 24 actions.
pub fn corpus_entries() -> Vec<CorpusEntry> {
    SEED_CORPUS
        .iter()
        .enumerate()
        .map(|(index, &seed)| {
            let (ticks, units, actions) = match index % 3 {
                0 => (100, 4, 8),
                1 => (300, 8, 16),
                _ => (600, 12, 24),
            };
            CorpusEntry {
                recipe: CORPUS_RECIPE_VERSION,
                seed,
                ticks,
                units,
                actions,
            }
        })
        .collect()
}

/// Looks up the corpus entry for a seed, if it is in the fixed corpus.
pub fn corpus_entry_for_seed(seed: u64) -> Option<CorpusEntry> {
    corpus_entries()
        .into_iter()
        .find(|entry| entry.seed == seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn corpus_stays_within_generation_ceilings() {
        let limits = crate::fuzz_scenario::default_limits();
        let entries = corpus_entries();
        assert_eq!(entries.len(), SEED_CORPUS.len());
        for (index, entry) in entries.iter().enumerate() {
            assert_eq!(entry.recipe, CORPUS_RECIPE_VERSION, "index={index}");
            assert!(
                u128::from(entry.ticks) <= u128::from(limits.max_ticks),
                "index={index} ticks={} over limit={}",
                entry.ticks,
                limits.max_ticks,
            );
            assert!(
                entry.units <= limits.max_units,
                "index={index} units={} over limit={}",
                entry.units,
                limits.max_units,
            );
            assert!(
                entry.actions <= limits.max_actions,
                "index={index} actions={} over limit={}",
                entry.actions,
                limits.max_actions,
            );
            let work = u128::from(entry.ticks) * u128::from(entry.units as u64 + 1)
                + u128::from(entry.actions as u64);
            assert!(
                work <= u128::from(limits.max_work),
                "index={index} work={work} over limit={}",
                limits.max_work,
            );
        }
    }

    #[test]
    fn corpus_generation_succeeds_within_limits() {
        let corpus = crate::fuzz_scenario::generate_corpus()
            .expect("fixed corpus must generate within default limits");
        assert_eq!(corpus.len(), SEED_CORPUS.len());
        for (seed, rpl) in &corpus {
            assert!(corpus_entry_for_seed(*seed).is_some());
            assert!(!rpl.is_empty(), "seed={seed}");
            assert!(
                rpl.len() <= crate::fuzz_scenario::default_limits().max_output_bytes,
                "seed={seed} output={} over limit={}",
                rpl.len(),
                crate::fuzz_scenario::default_limits().max_output_bytes,
            );
        }
    }

    /// The scenario body with the identity lines removed.
    ///
    /// The name carries the seed and the map tag is picked from the seed, so
    /// neither alone can show that two entries differ: a generator that
    /// ignored the seed would still give every entry its own name and map.
    /// What is left are the simulation inputs and the expectation.
    fn body_of(rpl: &str) -> String {
        rpl.lines()
            .filter(|line| !line.starts_with("scenario ") && !line.starts_with("map "))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn every_corpus_entry_regenerates_byte_for_byte() {
        let domains = crate::fuzz_scenario::default_domains();
        let limits = crate::fuzz_scenario::default_limits();
        for (index, entry) in corpus_entries().iter().enumerate() {
            let generate = || {
                crate::fuzz_scenario::generate_one(
                    entry.seed,
                    index,
                    entry.ticks,
                    entry.units,
                    entry.actions,
                    &domains,
                    limits,
                )
                .map(|(rpl, _)| rpl)
            };
            let first = generate().unwrap();
            let second = generate().unwrap();
            assert_eq!(first, second, "seed={} index={index}", entry.seed);
        }
    }

    #[test]
    fn every_corpus_entry_generates_a_distinct_scenario_body() {
        // A corpus whose entries all generated the same scenario would test
        // one input thirty-two times.
        let domains = crate::fuzz_scenario::default_domains();
        let limits = crate::fuzz_scenario::default_limits();
        let mut seen: BTreeMap<String, u64> = BTreeMap::new();
        for (index, entry) in corpus_entries().iter().enumerate() {
            let (rpl, _) = crate::fuzz_scenario::generate_one(
                entry.seed,
                index,
                entry.ticks,
                entry.units,
                entry.actions,
                &domains,
                limits,
            )
            .unwrap();
            let body = body_of(&rpl);
            assert!(
                seen.insert(body, entry.seed).is_none(),
                "seed={} index={index} repeats another entry's scenario body",
                entry.seed,
            );
        }
        assert_eq!(seen.len(), SEED_CORPUS.len());
    }

    #[test]
    fn every_corpus_entry_traces_byte_identically_across_runs() {
        let domains = crate::fuzz_scenario::default_domains();
        let limits = crate::fuzz_scenario::default_limits();
        for (index, entry) in corpus_entries().iter().enumerate() {
            let (rpl, _) = crate::fuzz_scenario::generate_one(
                entry.seed,
                index,
                entry.ticks,
                entry.units,
                entry.actions,
                &domains,
                limits,
            )
            .unwrap();
            let parsed = crate::Rpl::parse(&rpl).unwrap();
            let expected = parsed.run().unwrap().encode();
            assert_eq!(
                parsed.run().unwrap().encode(),
                expected,
                "seed={} index={index}",
                entry.seed,
            );
        }
    }
}

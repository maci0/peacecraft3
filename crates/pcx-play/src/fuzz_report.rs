//! Auto-filed divergence reports (PLAN §3.5, Tier C).
//!
//! Formatting only: this module turns a compared pair of traces into a
//! TRACKER-ready markdown snippet with the minimal reproducing seed. It
//! never edits `TRACKER.md` itself — the tracker stays append-only across
//! agents (see `docs/re-notes/parallel-ownership.md`), so the caller
//! appends the returned snippet as one entry.
//!
//! Coordination: comparison stays with the `mechanics` lane (`diff`,
//! `trace`); seed minimization stays with `tests/minimize.rs` in that
//! lane. The "minimal reproducing seed" here is the corpus seed that
//! produced the diverging RPL, plus the corpus recipe version that pins
//! the generation domains. Deeper input minimization (fewer units,
//! fewer actions, shorter horizons) hands off to that lane.

use crate::fuzz_corpus::CORPUS_RECIPE_VERSION;
use crate::trace::Trace;
use crate::{Divergence, fnv1a64};

/// Maximum RPL bytes embedded in a filing.
///
/// Collection output stays bounded: longer scenarios are referenced by
/// their recipe + seed + hash, with only a prefix quoted. The full text
/// is reproducible from [`crate::fuzz_scenario::generate_one`].
pub const MAX_RPL_SNIPPET_BYTES: usize = 2_048;

/// Maximum markdown bytes for one filing.
pub const MAX_FILING_BYTES: usize = 4_096;

/// A compared pair ready to file.
#[derive(Debug, Clone)]
pub struct DivergenceFiling<'a> {
    /// Fuzz scenario name (for example `fuzz.s7.3`).
    pub scenario: &'a str,
    /// Synthetic map identity recorded in the trace header.
    pub map: &'a str,
    /// Corpus seed that generated the diverging scenario.
    pub seed: u64,
    /// Corpus index (the `index` passed to `generate_one`).
    pub corpus_index: usize,
    /// Full generated RPL text (quoted only up to a bound).
    pub rpl: &'a str,
    /// First divergence between the two traces.
    pub divergence: &'a Divergence,
    /// Left (baseline) trace.
    pub left: &'a Trace,
    /// Right (candidate) trace.
    pub right: &'a Trace,
}

/// Bounded artifact file name for a divergence.
///
/// Always of the form `diverge-fuzz-s<seed>-<index>-<tick|header>.rpl`,
/// where each component is already name-safe. No caller input lands in
/// the path unescaped: the scenario name is not used.
pub fn artifact_name(seed: u64, corpus_index: usize, divergence: &Divergence) -> String {
    let tick = match divergence.tick {
        Some(t) => t.to_string(),
        None => "header".to_string(),
    };
    format!("diverge-fuzz-s{seed}-{corpus_index}-{tick}.rpl")
}

/// Formats a TRACKER-ready markdown snippet for a divergence.
///
/// The snippet carries the minimal reproducing seed, the recipe version,
/// both trace hashes, the one-line `pcx-diff` report and a bounded RPL
/// prefix. Output is truncated to [`MAX_FILING_BYTES`] with a trailing
/// marker so collection stays bounded.
pub fn format_tracker_entry(filing: &DivergenceFiling<'_>) -> String {
    let left_hash = filing.left.canonical_hash();
    let right_hash = filing.right.canonical_hash();
    let rpl_hash = fnv1a64(filing.rpl.as_bytes());
    let artifact = artifact_name(filing.seed, filing.corpus_index, filing.divergence);
    let snippet = rpl_snippet(filing.rpl);
    let mut out = format!(
        "- [~] Tier C fuzz divergence (recipe {recipe}): `{scenario}` on `{map}` \
(seed {seed}, corpus index {index}, RPL fnv1a64 `{rpl_hash:016x}`). \
`pcx-diff`: {divergence}. Baseline hash `{left_hash:016x}`, candidate hash `{right_hash:016x}`. \
Reproduce: regenerate the corpus entry (recipe {recipe}, seed {seed}, index {index}), \
run both builds, diff the traces; full RPL in `{artifact}` (RPL fnv1a64 `{rpl_hash:016x}`). \
RPL prefix:\n```\n{snippet}```\n",
        recipe = CORPUS_RECIPE_VERSION,
        scenario = filing.scenario,
        map = filing.map,
        seed = filing.seed,
        index = filing.corpus_index,
        rpl_hash = rpl_hash,
        divergence = filing.divergence,
        left_hash = left_hash,
        right_hash = right_hash,
        artifact = artifact,
        snippet = snippet,
    );
    if out.len() > MAX_FILING_BYTES {
        out.truncate(MAX_FILING_BYTES);
        out.push_str("\n…(truncated to filing bound)\n");
    }
    out
}

fn rpl_snippet(rpl: &str) -> &str {
    if rpl.len() <= MAX_RPL_SNIPPET_BYTES {
        return rpl;
    }
    let mut end = MAX_RPL_SNIPPET_BYTES;
    while end > 0 && !rpl.is_char_boundary(end) {
        end -= 1;
    }
    let prefix = &rpl[..end];
    match prefix.rfind('\n') {
        Some(index) => &prefix[..index + 1],
        None => prefix,
    }
}

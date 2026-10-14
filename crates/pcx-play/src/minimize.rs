//! Bounded differential seed minimization for Tier C fuzzing.
//!
//! Tier C fuzzing runs generated or scenario variants against two implementations
//! (such as an instrumentation baseline and an engine build, or two candidate simulation
//! models) to isolate behavioral divergences. When a failure is found on an arbitrary
//! initial seed, this module finds a verified smaller reproducing seed within an
//! explicitly bounded candidate search scope.
//!
//! # Core Principles
//!
//! 1. **Dual Execution Oracle**: For each evaluated seed candidate, both implementations
//!    must be freshly executed with that candidate seed. A candidate trace is never
//!    compared against a static baseline from a different seed or header.
//! 2. **Failure Reproduction Gate**: The original failing seed must reproduce the target
//!    failure signature before any search begins. If the original seed passes or diverges
//!    on an unrelated signature, search is refused.
//! 3. **Typed Error Preservation**: Oracle evaluation errors, stale traces, and incompatible
//!    trace identities (e.g. mismatched scenario or map names) remain typed errors and
//!    never pass as reproduced failures.
//! 4. **Signature Preservation**: Only divergences matching the requested failure predicate
//!    (defaulting to matching field identity) count as reproducing the defect, preventing
//!    early unrelated divergences from contaminating minimization.
//! 5. **Deterministic Deduplication**: Candidate seeds are evaluated in deterministic order.
//!    Duplicate seeds and the already-verified original seed are skipped without consuming
//!    oracle evaluation budget.
//! 6. **Bounded Budgets**: Two independent budgets govern search:
//!    - **Inspection Budget**: Maximum inspections (original verification and every yielded
//!      candidate, including duplicates, consume 1).
//!      This bounds iteration over infinite corpora and prevents unbounded loops.
//!    - **Oracle Budget**: Maximum number of fresh dual-execution oracle calls
//!      (the original verification and each unique candidate invocation consume 1).
//!
//!    Both budgets are hard ceilings; the search stops at the first exhausted budget.
//! 7. **Local Verification**: The returned minimum seed is verified within the checked
//!    candidate corpus only. No claim is made of a global minimum across all `u64`
//!    values, nor does it assume seed monotonicity or native RNG equivalence.

use std::collections::BTreeSet;

use crate::diff::{Divergence, first_divergence};
use crate::{Trace, TraceHeader};

/// Criteria used to determine whether a candidate divergence reproduces the original failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FailureSignature {
    /// Divergence must occur on the same field path (e.g. `unit[0].x` or `timer[0].name`).
    #[default]
    SameField,
    /// Divergence must occur on both the same tick index and the same field path.
    SameTickAndField,
    /// Divergence must match exactly across tick, field path, left value, and right value.
    Exact,
}

/// Trait for predicates that evaluate whether a candidate divergence reproduces the target failure.
pub trait DivergencePredicate {
    /// Returns `true` if `candidate` matches the requested failure characterized by `original`.
    fn matches(&self, candidate: &Divergence, original: &Divergence) -> bool;
}

impl DivergencePredicate for FailureSignature {
    fn matches(&self, candidate: &Divergence, original: &Divergence) -> bool {
        match self {
            Self::SameField => candidate.field == original.field,
            Self::SameTickAndField => {
                candidate.tick == original.tick && candidate.field == original.field
            }
            Self::Exact => candidate == original,
        }
    }
}

impl<F> DivergencePredicate for F
where
    F: Fn(&Divergence, &Divergence) -> bool,
{
    fn matches(&self, candidate: &Divergence, original: &Divergence) -> bool {
        self(candidate, original)
    }
}

/// Reasons why two traces in a differential pair are incompatible or stale.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TracePairError {
    #[error(
        "stale trace seed: expected {expected_seed}, left trace has {left_seed}, right trace has {right_seed}"
    )]
    StaleSeed {
        expected_seed: u64,
        left_seed: u64,
        right_seed: u64,
    },
    #[error("incompatible format version: left {left}, right {right}")]
    FormatVersionMismatch { left: u32, right: u32 },
    #[error("incompatible scenario identity: left {left:?}, right {right:?}")]
    ScenarioMismatch { left: String, right: String },
    #[error("incompatible map identity: left {left:?}, right {right:?}")]
    MapMismatch { left: String, right: String },
    #[error("incompatible asset mode: left {left:?}, right {right:?}")]
    AssetModeMismatch { left: String, right: String },
    #[error("incompatible tick count: left {left}, right {right}")]
    TickCountMismatch { left: u64, right: u64 },
    #[error("malformed tick sequence: {0}")]
    MalformedTickSequence(String),
    #[error(
        "{side} trace uses unsupported format version {found}; supported version is {supported}"
    )]
    UnsupportedFormatVersion {
        side: &'static str,
        found: u32,
        supported: u32,
    },
    #[error("{side} trace has malformed header field {field}: {reason}")]
    MalformedHeader {
        side: &'static str,
        field: &'static str,
        reason: &'static str,
    },
    #[error("{side} trace has malformed {field} at tick {tick}: {reason}")]
    MalformedRecord {
        side: &'static str,
        tick: u64,
        field: &'static str,
        reason: &'static str,
    },
    #[error("incompatible header field: {field} (left={left:?}, right={right:?})")]
    HeaderFieldMismatch {
        field: String,
        left: String,
        right: String,
    },
}

// Seed is deliberately excluded: each oracle call checks freshness separately.
fn validate_identity(left: &TraceHeader, right: &TraceHeader) -> Result<(), TracePairError> {
    // Format version.
    if left.format_version != right.format_version {
        return Err(TracePairError::FormatVersionMismatch {
            left: left.format_version,
            right: right.format_version,
        });
    }
    // Scenario identity.
    if left.scenario != right.scenario {
        return Err(TracePairError::ScenarioMismatch {
            left: left.scenario.clone(),
            right: right.scenario.clone(),
        });
    }
    // Map identity.
    if left.map != right.map {
        return Err(TracePairError::MapMismatch {
            left: left.map.clone(),
            right: right.map.clone(),
        });
    }
    // Asset mode.
    if left.asset_mode != right.asset_mode {
        return Err(TracePairError::AssetModeMismatch {
            left: left.asset_mode.as_str().to_string(),
            right: right.asset_mode.as_str().to_string(),
        });
    }
    // Tick count.
    if left.tick_count != right.tick_count {
        return Err(TracePairError::TickCountMismatch {
            left: left.tick_count,
            right: right.tick_count,
        });
    }

    if left.fix_rows != right.fix_rows {
        return Err(TracePairError::HeaderFieldMismatch {
            field: "fix-rows".into(),
            left: format!("{:?}", left.fix_rows),
            right: format!("{:?}", right.fix_rows),
        });
    }
    Ok(())
}

/// Validates that both traces in a differential pair were freshly executed with `expected_seed`
/// and share compatible scenario, map, format version, asset mode, and tick count headers.
/// Also validates tick sequence ordering and bounds (tick labels must be 0..tick_count in order).
pub fn validate_trace_pair(
    expected_seed: u64,
    left: &Trace,
    right: &Trace,
) -> Result<(), TracePairError> {
    // Seed freshness check.
    if left.header.seed != expected_seed || right.header.seed != expected_seed {
        return Err(TracePairError::StaleSeed {
            expected_seed,
            left_seed: left.header.seed,
            right_seed: right.header.seed,
        });
    }
    validate_identity(&left.header, &right.header)?;

    // Header validation: format version, canonical names, fix row ordering.
    for (trace, side) in [(left, "left"), (right, "right")] {
        if trace.header.format_version != crate::trace::TRACE_VERSION {
            return Err(TracePairError::UnsupportedFormatVersion {
                side,
                found: trace.header.format_version,
                supported: crate::trace::TRACE_VERSION,
            });
        }
        for (field, name) in [
            ("scenario", trace.header.scenario.as_str()),
            ("map", trace.header.map.as_str()),
        ] {
            if !crate::is_valid_name(name) {
                return Err(TracePairError::MalformedHeader {
                    side,
                    field,
                    reason: "invalid canonical name",
                });
            }
        }
        for row in &trace.header.fix_rows {
            if !crate::is_valid_name(&row.id) {
                return Err(TracePairError::MalformedHeader {
                    side,
                    field: "fix",
                    reason: "invalid canonical name",
                });
            }
        }
        if trace
            .header
            .fix_rows
            .windows(2)
            .any(|rows| rows[0].id >= rows[1].id)
        {
            return Err(TracePairError::MalformedHeader {
                side,
                field: "fix",
                reason: "rows must have unique ascending ids",
            });
        }
    }

    // Tick sequence validation: ticks must be labeled 0..tick_count-1 in strict order.
    for (trace, side) in [(left, "left"), (right, "right")] {
        if trace.ticks.len() as u64 != trace.header.tick_count {
            return Err(TracePairError::MalformedTickSequence(format!(
                "{side}: declared tick_count={} but found {} tick records",
                trace.header.tick_count,
                trace.ticks.len()
            )));
        }
        let mut last_rng = None;
        let mut last_timer = None;
        for (i, rec) in trace.ticks.iter().enumerate() {
            if rec.tick != i as u64 {
                return Err(TracePairError::MalformedTickSequence(format!(
                    "{side}: tick[{i}] label is {} but expected {i}",
                    rec.tick
                )));
            }
            if rec.units.windows(2).any(|units| units[0].id >= units[1].id) {
                return Err(TracePairError::MalformedRecord {
                    side,
                    tick: rec.tick,
                    field: "unit ids",
                    reason: "not strictly increasing",
                });
            }
            for draw in &rec.rng_draws {
                if last_rng.is_some_and(|last| draw.index <= last) {
                    return Err(TracePairError::MalformedRecord {
                        side,
                        tick: rec.tick,
                        field: "rng index",
                        reason: "not strictly increasing across the run",
                    });
                }
                last_rng = Some(draw.index);
            }
            for timer in &rec.timers {
                if last_timer.is_some_and(|last| timer.seq <= last) {
                    return Err(TracePairError::MalformedRecord {
                        side,
                        tick: rec.tick,
                        field: "timer sequence",
                        reason: "not strictly increasing across the run",
                    });
                }
                if !crate::is_valid_name(&timer.name) {
                    return Err(TracePairError::MalformedRecord {
                        side,
                        tick: rec.tick,
                        field: "timer name",
                        reason: "invalid canonical name",
                    });
                }
                last_timer = Some(timer.seq);
            }
        }
    }

    Ok(())
}

/// Configuration for a bounded differential seed minimization search.
#[derive(Debug, Clone)]
pub struct MinimizationConfig<I, P = FailureSignature> {
    /// The starting seed known to produce a divergence between implementations.
    /// This seed must reproduce the failure before candidate search begins.
    pub original_seed: u64,
    /// Caller-supplied finite corpus or bounded search scope of candidate seeds.
    pub candidate_corpus: I,
    /// Maximum number of fresh dual-execution oracle invocations.
    /// Includes the verification of `original_seed`.
    pub max_oracle_evaluations: usize,
    /// Maximum inspections, including original verification and every yielded
    /// candidate (even duplicates). Checked before advancing the iterator.
    pub max_inspections: usize,
    /// Predicate specifying which candidate divergences match the target failure signature.
    pub predicate: P,
}

impl<I> MinimizationConfig<I, FailureSignature> {
    /// Creates a new minimization configuration with the default [`FailureSignature::SameField`].
    /// Uses `max_oracle_evaluations` for both oracle and inspection budgets.
    pub fn new(original_seed: u64, candidate_corpus: I, max_evaluations: usize) -> Self {
        Self {
            original_seed,
            candidate_corpus,
            max_oracle_evaluations: max_evaluations,
            max_inspections: max_evaluations,
            predicate: FailureSignature::SameField,
        }
    }
}

impl<I, P> MinimizationConfig<I, P> {
    /// Sets a custom divergence predicate.
    pub fn with_predicate<P2>(self, predicate: P2) -> MinimizationConfig<I, P2> {
        MinimizationConfig {
            original_seed: self.original_seed,
            candidate_corpus: self.candidate_corpus,
            max_oracle_evaluations: self.max_oracle_evaluations,
            max_inspections: self.max_inspections,
            predicate,
        }
    }

    /// Sets the maximum number of candidate inspections (bounds iteration over the corpus).
    pub fn with_max_inspections(mut self, max_inspections: usize) -> Self {
        self.max_inspections = max_inspections;
        self
    }

    /// Sets the maximum number of oracle evaluations (bounds dual-execution calls).
    pub fn with_max_oracle_evaluations(mut self, max_oracle_evaluations: usize) -> Self {
        self.max_oracle_evaluations = max_oracle_evaluations;
        self
    }
}

/// Completion status of a minimization search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimizationStatus {
    /// All candidates in the finite corpus were evaluated within budget.
    CorpusExhausted,
    /// Evaluation stopped because the maximum oracle evaluation budget was reached.
    BudgetExhausted,
    /// Evaluation stopped because the maximum inspection budget was reached.
    InspectionBudgetExhausted,
}

/// The verified outcome of a bounded differential seed minimization.
///
/// # Scope and Limitations
/// The returned [`minimum_seed`](MinimizationResult::minimum_seed) is a verified
/// minimum within the checked candidate corpus only. It makes no claim of being a global
/// minimum across the full `u64` seed space, nor does it prove native game PRNG behavior
/// or combat ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimizationResult {
    /// The original failing seed verified before search.
    pub original_seed: u64,
    /// The smallest reproducing seed found within checked candidates.
    pub minimum_seed: u64,
    /// The divergence produced by `original_seed`.
    pub original_divergence: Divergence,
    /// The divergence produced by `minimum_seed`.
    pub minimum_divergence: Divergence,
    /// Total number of oracle evaluations performed (including `original_seed`).
    pub oracle_evaluations: usize,
    /// Total inspections: original verification plus yielded candidates.
    pub inspections: usize,
    /// Completion status of the search.
    pub status: MinimizationStatus,
}

impl MinimizationResult {
    /// Returns `true` if the candidate corpus was fully evaluated within budget.
    pub fn is_complete(&self) -> bool {
        self.status == MinimizationStatus::CorpusExhausted
    }

    /// Returns `true` if the search terminated because the oracle evaluation budget was exhausted.
    pub fn is_oracle_budget_exhausted(&self) -> bool {
        self.status == MinimizationStatus::BudgetExhausted
    }

    /// Returns `true` if the search terminated because the inspection budget was exhausted.
    pub fn is_inspection_budget_exhausted(&self) -> bool {
        self.status == MinimizationStatus::InspectionBudgetExhausted
    }

    /// Returns `true` if a strictly smaller reproducing seed was found.
    pub fn reduced(&self) -> bool {
        self.minimum_seed < self.original_seed
    }
}

/// Errors returned during differential seed minimization.
///
/// Oracle errors, stale traces, and incompatible header identities are preserved as typed
/// errors rather than silently passing or being counted as reproduced failures.
#[derive(Debug, thiserror::Error)]
pub enum MinimizationError<E> {
    #[error("oracle evaluation budget must be greater than zero")]
    ZeroOracleEvaluationBudget,

    #[error("inspection budget must be greater than zero")]
    ZeroInspectionBudget,

    #[error("original seed {seed} did not reproduce any divergence between implementations")]
    OriginalSeedDidNotDiverge { seed: u64 },

    #[error(
        "original seed {seed} divergence does not match requested failure signature: {divergence}"
    )]
    OriginalSeedMismatchedSignature { seed: u64, divergence: Divergence },

    #[error("oracle failed on original seed {seed}: {source}")]
    OriginalSeedOracle {
        seed: u64,
        #[source]
        source: E,
    },

    #[error("original seed trace pair error: {0}")]
    OriginalSeedTracePair(#[from] TracePairError),

    #[error("oracle failed on candidate seed {candidate_seed}: {source}")]
    CandidateOracle {
        candidate_seed: u64,
        #[source]
        source: E,
    },

    #[error("candidate seed {candidate_seed} trace pair error: {error}")]
    CandidateTracePair {
        candidate_seed: u64,
        error: TracePairError,
    },
}

/// Minimizes a failing differential seed within an explicit candidate corpus and budgets.
///
/// # Requirements
/// - `config.max_oracle_evaluations` must be at least 1.
/// - `config.max_inspections` must be at least 1.
/// - `config.original_seed` is evaluated first and must reproduce the requested failure signature.
/// - `oracle` is called with candidate seeds to regenerate BOTH implementations freshly.
/// - Candidates that produce oracle errors, stale traces, or incompatible headers return typed errors.
/// - Duplicate seeds and `original_seed` are skipped without consuming oracle evaluation budget.
/// - Every candidate iteration (including duplicates and original_seed) consumes 1 inspection budget.
///
/// # Returns
/// A [`MinimizationResult`] containing the verified minimum seed within the checked corpus,
/// or a typed [`MinimizationError`].
pub fn minimize_differential_seed<I, P, F, E>(
    config: MinimizationConfig<I, P>,
    mut oracle: F,
) -> Result<MinimizationResult, MinimizationError<E>>
where
    I: IntoIterator<Item = u64>,
    P: DivergencePredicate,
    F: FnMut(u64) -> Result<(Trace, Trace), E>,
{
    if config.max_oracle_evaluations == 0 {
        return Err(MinimizationError::ZeroOracleEvaluationBudget);
    }
    if config.max_inspections == 0 {
        return Err(MinimizationError::ZeroInspectionBudget);
    }

    // 1. Verify that the original seed reproduces the requested failure.
    // This consumes 1 oracle evaluation and 1 inspection.
    let (left_orig, right_orig) =
        oracle(config.original_seed).map_err(|source| MinimizationError::OriginalSeedOracle {
            seed: config.original_seed,
            source,
        })?;

    validate_trace_pair(config.original_seed, &left_orig, &right_orig)?;

    let orig_div = first_divergence(&left_orig, &right_orig).ok_or(
        MinimizationError::OriginalSeedDidNotDiverge {
            seed: config.original_seed,
        },
    )?;

    if orig_div.tick.is_none() {
        return Err(MinimizationError::OriginalSeedTracePair(
            TracePairError::HeaderFieldMismatch {
                field: orig_div.field,
                left: orig_div.left,
                right: orig_div.right,
            },
        ));
    }

    if !config.predicate.matches(&orig_div, &orig_div) {
        return Err(MinimizationError::OriginalSeedMismatchedSignature {
            seed: config.original_seed,
            divergence: orig_div,
        });
    }

    let mut oracle_evaluations_used = 1usize;
    let mut inspections_used = 1usize;
    let mut best_seed = config.original_seed;
    let mut best_divergence = orig_div.clone();

    // Deduplication set: bounded by max_oracle_evaluations (only oracle evaluations add to it).
    let mut evaluated_seeds = BTreeSet::new();
    evaluated_seeds.insert(config.original_seed);

    let mut status = MinimizationStatus::CorpusExhausted;
    let mut corpus_iter = config.candidate_corpus.into_iter();

    // Bounded inspection loop: inspection budget checked BEFORE iterator advancement.
    loop {
        if inspections_used >= config.max_inspections {
            status = MinimizationStatus::InspectionBudgetExhausted;
            break;
        }
        if oracle_evaluations_used >= config.max_oracle_evaluations {
            status = MinimizationStatus::BudgetExhausted;
            break;
        }
        let Some(candidate_seed) = corpus_iter.next() else {
            break;
        };
        inspections_used += 1;

        // Deduplication: skip without consuming oracle budget.
        if evaluated_seeds.contains(&candidate_seed) {
            continue;
        }

        // This is a new unique seed to evaluate.
        oracle_evaluations_used += 1;
        evaluated_seeds.insert(candidate_seed);

        let (cand_left, cand_right) =
            oracle(candidate_seed).map_err(|source| MinimizationError::CandidateOracle {
                candidate_seed,
                source,
            })?;

        if let Err(err) = validate_trace_pair(candidate_seed, &cand_left, &cand_right) {
            return Err(MinimizationError::CandidateTracePair {
                candidate_seed,
                error: err,
            });
        }

        validate_identity(&left_orig.header, &cand_left.header).map_err(|error| {
            MinimizationError::CandidateTracePair {
                candidate_seed,
                error,
            }
        })?;

        let cand_div_opt = first_divergence(&cand_left, &cand_right);
        if let Some(cand_div) = cand_div_opt {
            if cand_div.tick.is_none() {
                return Err(MinimizationError::CandidateTracePair {
                    candidate_seed,
                    error: TracePairError::HeaderFieldMismatch {
                        field: cand_div.field,
                        left: cand_div.left,
                        right: cand_div.right,
                    },
                });
            }

            if config.predicate.matches(&cand_div, &orig_div) && candidate_seed < best_seed {
                best_seed = candidate_seed;
                best_divergence = cand_div;
            }
        }
    }

    Ok(MinimizationResult {
        original_seed: config.original_seed,
        minimum_seed: best_seed,
        original_divergence: orig_div,
        minimum_divergence: best_divergence,
        oracle_evaluations: oracle_evaluations_used,
        inspections: inspections_used,
        status,
    })
}

/// Helper generators for common finite candidate seed search scopes.
pub mod search_scope {
    /// Generates candidate seeds by clearing individual set bits of `original_seed`,
    /// from most significant bit down to least significant bit.
    pub fn bit_clearing(original_seed: u64) -> impl Iterator<Item = u64> {
        (0..64).rev().filter_map(move |bit| {
            let mask = 1u64 << bit;
            if (original_seed & mask) != 0 {
                Some(original_seed & !mask)
            } else {
                None
            }
        })
    }

    /// Generates candidate seeds by repeatedly halving the seed value until 0.
    pub fn halving(original_seed: u64) -> impl Iterator<Item = u64> {
        std::iter::successors(Some(original_seed / 2), |&s| {
            if s > 0 { Some(s / 2) } else { None }
        })
    }

    /// Generates a finite sequential scope of small seeds: `0..limit`.
    pub fn sequential_small(limit: u64) -> impl Iterator<Item = u64> {
        0..limit
    }

    /// Combines small seed candidates (`0..16`), bit-clearing candidates, and halving candidates.
    pub fn standard_reduction_scope(original_seed: u64) -> impl Iterator<Item = u64> {
        let small_limit = 16u64.min(original_seed);
        sequential_small(small_limit)
            .chain(bit_clearing(original_seed))
            .chain(halving(original_seed))
    }
}

//! Tier C generated scenarios (PLAN §3.5, Tier C).
//!
//! Harness convention, not game behavior. Random unit compositions and the
//! synthetic "terrain" tag below are caller policy over the existing
//! movement generator (`crates/pcx-play/src/generator.rs`); they are not the
//! original game's composition, terrain generation or RNG distribution.
//! Generation and execution each start a separate `pcx_sim::Rng` at the
//! supplied seed, and selection is reproducible modulo indexing. It is not
//! an unbiased distribution, and it must never be read as one.
//!
//! Coordination: the RPL grammar (`rpl`), the runner (`runner`), the
//! scenario types (`scenario`) and the trace codec (`trace`) stay with the
//! `mechanics` lane. This module only picks bounded [`MovementSpec`]
//! parameters from a seed and delegates emission to
//! [`crate::generator::generate`], so grammar changes there flow through
//! without an edit here.

use std::ops::RangeInclusive;

use pcx_sim::{Fx, Rng};

use crate::fuzz_corpus::CORPUS_RECIPE_VERSION;
use crate::generator::{ActionKind, Limits, MovementSpec, generate};
use crate::scenario::RunConfig;
use crate::trace::{AssetMode, FixRow};

/// Caller-supplied generation domains.
///
/// Every domain is caller policy. Owners are synthetic `u8` identifiers,
/// not a claim about the native game's player slots. Coordinates are raw
/// `Fx` words. Map names are synthetic identities recorded in the trace
/// header; they do not load an original map container.
#[derive(Debug, Clone)]
pub struct FuzzDomains<'a> {
    /// Candidate synthetic map identities; one is picked per scenario.
    pub maps: &'a [&'a str],
    /// Candidate owners; the per-unit and per-action owners are picked from
    /// these.
    pub owners: &'a [u8],
    /// Inclusive coordinate domain for spawns and move/select targets.
    pub x: RangeInclusive<Fx>,
    /// Inclusive coordinate domain for spawns and move/select targets.
    pub y: RangeInclusive<Fx>,
    /// Candidate action kinds; the per-action kind is picked from these.
    pub action_kinds: &'a [ActionKind],
}

/// Explicit generation/output ceilings for fuzz scenarios.
///
/// Mirrors the work model in `generator.rs`: work counts
/// `ticks * (units + 1) + actions`. These defaults admit the whole fixed
/// corpus in [`crate::fuzz_corpus`] with headroom for ad-hoc seeds.
pub fn default_limits() -> Limits {
    Limits {
        max_units: 16,
        max_actions: 32,
        max_ticks: 1_000,
        max_fix_rows: 8,
        max_work: 32_768,
        max_output_bytes: 16_384,
    }
}

/// Default synthetic domains for corpus generation.
///
/// Three map tags stand in for "random terrain" until map-container wiring
/// lands elsewhere: they only change the recorded map identity and the
/// coordinate domain is shared. Owners cover two synthetic players.
pub fn default_domains() -> FuzzDomains<'static> {
    use std::sync::LazyLock;
    static MAPS: &[&str] = &[
        "synthetic.fuzz.plain",
        "synthetic.fuzz.hills",
        "synthetic.fuzz.water",
    ];
    static OWNERS: &[u8] = &[0, 1];
    static KINDS: &[ActionKind] = &[
        ActionKind::Move,
        ActionKind::Select,
        ActionKind::Stop,
        ActionKind::Nop,
        ActionKind::RngDraw,
    ];
    static DOMAINS: LazyLock<FuzzDomains<'static>> = LazyLock::new(|| FuzzDomains {
        maps: MAPS,
        owners: OWNERS,
        x: RangeInclusive::new(-20, 20),
        y: RangeInclusive::new(-20, 20),
        action_kinds: KINDS,
    });
    FuzzDomains {
        maps: DOMAINS.maps,
        owners: DOMAINS.owners,
        x: DOMAINS.x.clone(),
        y: DOMAINS.y.clone(),
        action_kinds: DOMAINS.action_kinds,
    }
}

/// Generates one deterministic RPL text for `seed`.
///
/// `index` distinguishes multiple scenarios under the same seed; both feed
/// the scenario name (`fuzz.s<seed>.<index>`), so outputs never collide.
/// `ticks`, `units` and `actions` are the caller's bounded shape (see
/// [`crate::fuzz_corpus::CorpusEntry`]). The run configuration carries the
/// same seed and `vanilla` assets; fix rows are none by default and can be
/// added by the caller afterwards.
///
/// Errors are the underlying generator errors (invalid names, over-limit
/// shapes, output ceilings).
pub fn generate_one(
    seed: u64,
    index: usize,
    ticks: u64,
    units: usize,
    actions: usize,
    domains: &FuzzDomains<'_>,
    limits: Limits,
) -> Result<(String, RunConfig), crate::generator::Error> {
    let scenario_name = format!("fuzz.s{seed}.{index}");
    let mut picker = Rng::new(
        seed.wrapping_add(index as u64)
            .wrapping_add(u64::from(CORPUS_RECIPE_VERSION).wrapping_mul(0x9E37_79B9_7F4A_7C15)),
    );
    let map = domains.maps[(picker.next_u64() % domains.maps.len() as u64) as usize];
    let config = RunConfig {
        asset_mode: AssetMode::Vanilla,
        seed,
        fix_rows: Vec::<FixRow>::new(),
    };
    let spec = MovementSpec {
        scenario: &scenario_name,
        map,
        ticks,
        units,
        actions,
        owners: domains.owners,
        x: domains.x.clone(),
        y: domains.y.clone(),
        action_kinds: domains.action_kinds,
    };
    let rpl = generate(&spec, &config, limits)?;
    Ok((rpl, config))
}

/// Generates the RPL text for every entry of the fixed seed corpus.
///
/// Returns `(seed, rpl)` pairs in corpus order. Uses
/// [`default_domains`] and [`default_limits`]; any generator refusal is
/// returned as the error for that entry's shape.
pub fn generate_corpus() -> Result<Vec<(u64, String)>, crate::generator::Error> {
    let entries = crate::fuzz_corpus::corpus_entries();
    let domains = default_domains();
    let limits = default_limits();
    let mut out = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let (rpl, _) = generate_one(
            entry.seed,
            index,
            entry.ticks,
            entry.units,
            entry.actions,
            &domains,
            limits,
        )?;
        out.push((entry.seed, rpl));
    }
    Ok(out)
}

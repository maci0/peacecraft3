//! Caller-qualified RNG draw sites in execution order.
//!
//! The reviewed primary generator ([`super::state`]) advances one step per
//! nondegenerate call, but it does not remember *which* caller drew. The
//! retained startup history in `docs/re-notes/original-rng-control.md` shows
//! why that matters: one run's primary writes before the first recorded script
//! real split into player-slot race-resolution draws and `CAbilityWander`
//! draws reached through `CreateUnit`. That observed total is one run's
//! history, not a universal warmup, so this log records the actual call order
//! of *this* session instead of installing a fixed skip. It carries no seeds,
//! counts, tables or output values; a reseed or session replacement clears it
//! alongside the generator state.

use super::state::StateError;

/// Where one admitted RNG call came from.
///
/// Variants name reviewed consumer roles. They carry caller identity only:
/// no seed, table, count or output value is stored here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallSite {
    /// One player-slot race-resolution draw, keyed by engine slot row.
    RaceResolution { slot: u32 },
    /// One `CAbilityWander` range/fraction draw for a created unit, keyed by
    /// creation order within this session.
    Wander { unit: u32 },
    /// One script `GetRandomInt` draw, in JASS execution order.
    ScriptInteger,
    /// One script `GetRandomReal` draw, in JASS execution order.
    ScriptReal,
    /// One die from an already retained secondary stream.
    SecondaryDie { channel: usize },
    /// One fractional draw from an already retained secondary stream.
    SecondaryReal { channel: usize },
    /// Any other admitted gameplay consumer, named by its caller.
    Gameplay { caller: Box<str> },
}

impl CallSite {
    /// Name an ad-hoc gameplay caller without hiding allocation failure.
    pub fn gameplay(caller: &str) -> Result<Self, StateError> {
        let mut owned = String::new();
        owned
            .try_reserve(caller.len())
            .map_err(|_| StateError::Order)?;
        owned.push_str(caller);
        Ok(Self::Gameplay {
            caller: owned.into_boxed_str(),
        })
    }
}

/// One admitted call in execution order, with post-call primary and selected secondary words.
///
/// The word mirrors [`super::state::State::primary_word`] right after the
/// call, so a replay can verify exact state progression alongside site order.
/// Refused calls push no record. Degenerate unseeded calls carry no word, and
/// the field stays optional to match the accessor. Equal-bound calls that
/// consume no draw are still recorded: they are caller sequence, even though
/// the generator did not advance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawRecord {
    pub site: CallSite,
    pub primary_word: Option<u32>,
    /// Selected secondary word after a secondary call; absent for primary calls.
    pub secondary_word: Option<u32>,
}

/// Caller sequence for one session, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DrawLog {
    records: Vec<DrawRecord>,
}

impl DrawLog {
    /// An empty caller sequence; reseeds and replacements start here.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Reserve one record slot before drawing, so a refused draw leaves no
    /// phantom entry and a budget refusal leaves prior state untouched.
    pub(crate) fn reserve(&mut self) -> Result<(), StateError> {
        self.records.try_reserve(1).map_err(|_| StateError::Order)?;
        Ok(())
    }

    /// Append after a completed draw. Capacity was reserved up front, so this
    /// cannot fail and cannot disturb the generator.
    pub(crate) fn push(&mut self, record: DrawRecord) {
        self.records.push(record);
    }

    /// Admitted calls in the order they executed.
    pub fn records(&self) -> &[DrawRecord] {
        &self.records
    }

    /// Drop the caller sequence. Callers invoke this exactly where the
    /// generator state is replaced: executed reseeds and session teardown.
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Drop records appended after a checkpoint restore point. Restoring an
    /// undone draw must also undo its caller record, or the sequence would
    /// describe draws the generator no longer holds.
    pub(crate) fn truncate(&mut self, len: usize) {
        self.records.truncate(len);
    }

    /// Whether the recorded caller order equals an expected site order.
    /// Length and pairwise sites must match; words are compared separately
    /// through [`super::state::State::primary_word`].
    pub fn replay_matches(&self, expected: &[CallSite]) -> bool {
        self.records.len() == expected.len()
            && self
                .records
                .iter()
                .zip(expected)
                .all(|(record, site)| &record.site == site)
    }
}

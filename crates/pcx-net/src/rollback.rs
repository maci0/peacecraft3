//! Bounded client-side history, and the budget a single correction may spend.
//!
//! Rolling back is not free, and in this design it is not unconditional. Two
//! bounds keep it honest:
//!
//! - **Memory.** A bounded ring of past states, because a client that kept every
//!   state it ever predicted would grow without limit and a browser tab would
//!   eventually fall over.
//! - **Work.** A per-correction re-simulation budget, because a correction that
//!   lands far behind would otherwise make the client replay an unbounded run of
//!   ticks inside one frame, which is worse than the teleport it replaced.
//!
//! When either bound is exceeded the client adopts authority whole. That is the
//! path this design had before rollback existed, so the fallback is not a
//! second-class path: it is always correct, always cheap, and always reachable.
//!
//! Nothing here decides *what* is true. The base for every rollback is the
//! server's world; re-simulation only ever replays the client's own inputs
//! through [`pcx_sim::step`], the same deterministic step the server runs. A
//! rollback therefore computes what the server *will* produce — it never lets a
//! client assert a state the server did not send.

use std::collections::VecDeque;

use pcx_sim::{Rng, World};

/// Past states kept by default: 30 ticks is 750 ms at the project's 25 ms step.
///
/// Chosen to cover a normal round trip plus its jitter. A correction that needs
/// to reach further back than this is not a correction, it is a resync, and the
/// server has a [`crate::Server::resync_to`] for that.
pub const DEFAULT_ROLLBACK_TICKS: usize = 30;

/// Default per-correction re-simulation budget, in ticks.
///
/// Equal to the default window on purpose: there is no point replaying more
/// ticks than the ring can hold frames for, because a frame older than the
/// window is exactly what makes a second correction unreplayable.
pub const DEFAULT_MAX_RESIM_TICKS: usize = 30;

/// Memory and work bounds for client-side reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RollbackLimits {
    /// How many past client states to retain. Memory bound; 0 disables
    /// rollback entirely and leaves every correction as a snap.
    pub history_ticks: usize,
    /// The most ticks a single correction may re-simulate. Work bound.
    pub max_resim_ticks: usize,
}

impl Default for RollbackLimits {
    fn default() -> Self {
        Self {
            history_ticks: DEFAULT_ROLLBACK_TICKS,
            max_resim_ticks: DEFAULT_MAX_RESIM_TICKS,
        }
    }
}

/// One retained past state, with the random stream that accompanied it.
///
/// The RNG is retained alongside the world because re-simulation is only
/// reproducible if it replays *both*. `step` threads the stream through rather
/// than storing it in the world, so a bare world cannot say which stream
/// produced it — and the server does not put its stream on the wire. The frame
/// this client recorded at tick T is therefore the only stream it can honestly
/// replay from, which is what makes the window load-bearing rather than a
/// convenience.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    tick: u64,
    world: World,
    rng: Rng,
}

impl Frame {
    /// The tick this state is the start of, i.e. the state *before* the input
    /// for that tick was applied.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn rng(&self) -> &Rng {
        &self.rng
    }
}

/// A bounded ring of past client states.
///
/// Entries are pushed in tick order and the oldest is dropped once the ring is
/// full, so the retained span is always the most recent `window` ticks. A window
/// of 0 retains nothing, which is a legitimate configuration: it is how a caller
/// asks for "never roll back", and every correction becomes a snap.
#[derive(Debug, Clone, Default)]
pub struct TickHistory {
    window: usize,
    frames: VecDeque<Frame>,
}

impl TickHistory {
    pub fn new(window: usize) -> Self {
        Self {
            window,
            frames: VecDeque::with_capacity(window.min(64)),
        }
    }

    /// The configured capacity, in ticks.
    pub fn window(&self) -> usize {
        self.window
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// The oldest tick still retained, if any.
    pub fn oldest(&self) -> Option<u64> {
        self.frames.front().map(|f| f.tick)
    }

    /// The newest tick still retained, if any.
    pub fn newest(&self) -> Option<u64> {
        self.frames.back().map(|f| f.tick)
    }

    /// Retains the state at `tick`, discarding newer frames and replacing any
    /// frame that already describes that tick.
    ///
    /// Replacing rather than appending matters because a client at tick T that
    /// is corrected and re-simulated walks back over ticks it already recorded.
    /// Those frames describe the *divergent* trajectory, so overwriting them as
    /// the replay passes keeps the ring a record of the current trajectory
    /// instead of a record of a route the client abandoned.
    pub fn record(&mut self, tick: u64, world: &World, rng: &Rng) {
        if self.window == 0 {
            return;
        }
        self.truncate_after(tick);
        if let Some(newest) = self.frames.back_mut()
            && newest.tick == tick
        {
            newest.world = world.clone();
            newest.rng = rng.clone();
            return;
        }
        while self.frames.len() >= self.window {
            self.frames.pop_front();
        }
        self.frames.push_back(Frame {
            tick,
            world: world.clone(),
            rng: rng.clone(),
        });
    }

    /// The frame for `tick`, or `None` when it has aged out or was never kept.
    pub fn get(&self, tick: u64) -> Option<&Frame> {
        self.frames.iter().find(|f| f.tick == tick)
    }

    /// Drops every frame after `tick`.
    ///
    /// Called whenever the trajectory past a tick is abandoned — by a snap, or by
    /// a replay that ran out of inputs — so a stale frame can never be handed to
    /// a later rollback as if it were current.
    pub fn truncate_after(&mut self, tick: u64) {
        while self.frames.back().is_some_and(|f| f.tick > tick) {
            self.frames.pop_back();
        }
    }
}

/// Why a correction could not be rolled back and authority was adopted instead.
///
/// Every variant is a case where the client cannot reproduce what the server
/// would compute, and in every one of them adopting the server's world whole is
/// the only answer that does not assert something the server never said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapReason {
    /// The server has acknowledged nothing, so there is no evidence about which
    /// of the client's inputs have been consumed. Replaying them would be the
    /// client guessing at the server's clock.
    Unconfirmed,
    /// The correction's tick has aged out of the retained window, so the only
    /// random stream valid at that tick is gone.
    OutsideWindow,
    /// Re-simulating would exceed the per-correction budget.
    ResimBudget,
    /// The correction sits behind the server's own clock, leaving inputs this
    /// client no longer holds between the snapshot and the last acknowledgement.
    InputGap,
}

/// What a reconciliation actually did.
///
/// Reconciliation is reportable rather than silent because a correction the
/// caller cannot see is indistinguishable from a bug: either the world moved or
/// it did not, and the difference matters to whoever is watching the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Correction {
    /// Positions agreed at the same tick. Authority's other fields were adopted
    /// without running another simulation step.
    Aligned,
    /// Authority was restored at `tick` and `resimulated` unacknowledged ticks
    /// of the client's own input were replayed on top of it.
    ///
    /// `resimulated` can be fewer than the ticks that were predicted past
    /// `tick`, which means the replay ran out of inputs to apply; the client is
    /// then correct but shorter of where it had predicted.
    RolledBack { tick: u64, resimulated: u32 },
    /// Authority was adopted whole, because it could not be replayed forward.
    Snapped { tick: u64, reason: SnapReason },
    /// The correction was older than authority already accepted, so it was
    /// ignored. A late duplicate is not a disagreement.
    Stale { tick: u64 },
    /// The snapshot names a different movement admission from this client.
    /// Prediction was left as it was, and no step ran.
    AdmissionMismatch { tick: u64 },
}

impl Correction {
    /// Whether this correction moved the client by replaying input.
    pub fn rolled_back(&self) -> bool {
        matches!(self, Self::RolledBack { .. })
    }

    /// Whether authority was adopted whole, i.e. the visible teleport path.
    pub fn snapped(&self) -> bool {
        matches!(self, Self::Snapped { .. })
    }
}

/// The outcome of one reconciliation.
#[derive(Debug, Clone)]
pub struct Reconciliation {
    /// The received world, with this client's movement admission restored when compatible.
    pub snapshot: World,
    /// Positional distance between the client's prediction and that world.
    pub divergence: u64,
    /// What was done about it.
    pub correction: Correction,
    /// Where the client's prediction ended up.
    pub predicted_tick: u64,
}

/// How far apart two worlds are, in Manhattan distance over shared units.
///
/// The metric is positional and unit-scoped, matching what it has always been:
/// it sums per-unit distance for units present in both worlds and ignores the
/// rest. Two consequences are worth stating rather than leaving to be discovered
/// — a difference in `move_target` alone does not read as divergence, and a unit
/// missing from one side contributes nothing. Neither breaks the design, because
/// movement is resolved after commands and so a mismatched target shows up as a
/// positional divergence on a later correction.
/// The aggregate saturates at `u64::MAX` if the distance cannot fit the report.
pub fn positional_divergence(predicted: &World, authoritative: &World) -> u64 {
    predicted
        .units
        .iter()
        .filter_map(|p| {
            authoritative
                .unit(p.id)
                .map(|a| u64::from(p.x.abs_diff(a.x)) + u64::from(p.y.abs_diff(a.y)))
        })
        .fold(0, u64::saturating_add)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_world(tick: u64) -> World {
        let mut w = World::empty();
        w.tick = tick;
        w.spawn(0, tick as i32, 0);
        w
    }

    #[test]
    fn a_history_keeps_at_most_its_window() {
        let mut h = TickHistory::new(4);
        let rng = Rng::new(1);
        for t in 0..10u64 {
            h.record(t, &frame_world(t), &rng);
        }
        assert_eq!(h.len(), 4, "the ring must not grow past its window");
        assert_eq!(h.oldest(), Some(6), "the most recent ticks are retained");
        assert_eq!(h.newest(), Some(9));
        assert!(h.get(5).is_none(), "an aged-out frame is gone");
        assert_eq!(h.get(7).unwrap().tick(), 7);
    }

    #[test]
    fn a_zero_window_keeps_nothing() {
        let mut h = TickHistory::new(0);
        let rng = Rng::new(1);
        h.record(0, &frame_world(0), &rng);
        assert!(
            h.is_empty(),
            "a zero window is how a caller disables rollback"
        );
        assert!(h.get(0).is_none());
    }

    #[test]
    fn re_recording_a_tick_replaces_rather_than_grows_the_ring() {
        let mut h = TickHistory::new(4);
        let rng = Rng::new(1);
        for t in 0..4u64 {
            h.record(t, &frame_world(t), &rng);
        }
        let corrected = frame_world(3);
        h.record(3, &corrected, &rng);
        assert_eq!(h.len(), 4, "replaying a tick must not consume a slot");
        assert_eq!(
            h.get(3).unwrap().world(),
            &corrected,
            "the frame must describe the current trajectory, not the abandoned one"
        );
    }

    #[test]
    fn a_frame_for_an_abandoned_trajectory_is_dropped() {
        let mut h = TickHistory::new(8);
        let rng = Rng::new(1);
        for t in 0..6u64 {
            h.record(t, &frame_world(t), &rng);
        }
        h.truncate_after(3);
        assert_eq!(h.oldest(), Some(0));
        assert_eq!(h.newest(), Some(3));
        assert!(h.get(4).is_none(), "a stale frame must not survive");
    }

    #[test]
    fn recording_an_earlier_tick_discards_the_abandoned_tail() {
        let mut history = TickHistory::new(4);
        let rng = Rng::new(1);
        for tick in 0..4 {
            history.record(tick, &frame_world(tick), &rng);
        }
        let mut corrected = frame_world(1);
        corrected.units[0].x = -17;
        history.record(1, &corrected, &rng);
        assert_eq!(history.len(), 2);
        assert_eq!(history.oldest(), Some(0));
        assert_eq!(history.newest(), Some(1));
        assert_eq!(history.get(1).unwrap().world(), &corrected);
        assert!(history.get(2).is_none());
        assert!(history.get(3).is_none());
    }

    #[test]
    fn divergence_is_positional_and_ignores_unknown_units() {
        let mut a = World::empty();
        a.spawn(0, 0, 0);
        a.spawn(1, 100, 0);
        let mut b = a.clone();
        b.units[0].x = 30;
        assert_eq!(positional_divergence(&a, &b), 30);
        // A unit the authority does not have cannot contribute a distance, and
        // a unit it has but the prediction lacks is never examined.
        let mut c = World::empty();
        c.spawn(0, 0, 0);
        assert_eq!(positional_divergence(&a, &c), 0);
    }
}

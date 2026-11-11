//! Movement speed state of one unit, as the reviewed client keeps it in the movement object at
//! `unit+0x3f8`. The decode and its evidence are in `docs/re-notes/unit-move-speed-model.md`.
//!
//! The speed a unit walks at is
//! `limit((max(bonuses) + base) * multiplier)`, where
//!
//! - `base` is `[+0x74]`: the unit-type `spd` at creation, rewritten by `SetUnitMoveSpeed`;
//! - `bonuses` are the flat values of the unit's speed-bonus abilities (`AIms`), of which only the
//!   largest counts, starting from zero;
//! - `multiplier` is `[+0x7c]`: 1.0 when neutral, shifted by signed additive deltas that buffs add
//!   when they apply and subtract when they end (slow, frost). A sum within [`SNAP`] of 1.0 reads
//!   as exactly 1.0. A multiplier at or below zero is allowed; the limit then sets the speed;
//! - `limit` raises the result to the lower bound and then lowers it to the upper bound.
use pcx_objects::editor::misc::{ENGINE_MAX_SPEED, ENGINE_MIN_SPEED};
use pcx_script::HandleId;

/// The multiplier of a unit nothing slows or hastens (`0x112d810`).
const NEUTRAL: f32 = 1.0;
/// A multiplier this close to [`NEUTRAL`] is stored as exactly [`NEUTRAL`] (`0x10db054`).
const SNAP: f32 = 0.001;

/// Lower and upper speed bound of one unit (`0xb681b0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SpeedBounds {
    min: f32,
    max: f32,
}

impl SpeedBounds {
    /// The bounds of a unit type. A unit-type `minSpd` or `maxSpd` of zero defers to the `Misc`
    /// bound of the unit's class (`misc`, already clamped when the profiles were read). A nonzero
    /// value is clamped into the client's hard range first.
    pub(super) fn resolve(type_min: f32, type_max: f32, misc: (f32, f32)) -> Self {
        let own = |value: f32, fallback: f32| {
            if value == 0.0 {
                fallback
            } else {
                value.clamp(ENGINE_MIN_SPEED, ENGINE_MAX_SPEED)
            }
        };
        Self {
            min: own(type_min, misc.0),
            max: own(type_max, misc.1),
        }
    }

    /// Raises `speed` to the minimum, then lowers it to the maximum, so the maximum wins when
    /// the unit type gives a minimum above its maximum.
    fn apply(self, speed: f32) -> f32 {
        let raised = if self.min > speed { self.min } else { speed };
        if raised > self.max { self.max } else { raised }
    }
}

/// Base speed, bonus list, additive multiplier and bounds of one unit.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MoveSpeed {
    /// `None` for a unit whose `spd` is absent and that nothing has set.
    base: Option<f32>,
    multiplier: f32,
    /// One entry per source (a held item), each the flat bonus that source grants.
    bonuses: Vec<(HandleId, f32)>,
    /// `None` when the `Misc` limits were not admitted: the speed is then not limited.
    bounds: Option<SpeedBounds>,
}

impl MoveSpeed {
    pub(super) fn new(base: Option<f32>, bounds: Option<SpeedBounds>) -> Self {
        Self {
            base,
            multiplier: NEUTRAL,
            bonuses: Vec::new(),
            bounds,
        }
    }

    pub(super) fn bounded(&self) -> bool {
        self.bounds.is_some()
    }

    pub(super) fn set_base(&mut self, base: f32) {
        self.base = Some(base);
    }

    /// Adds `delta` to the multiplier. Buffs call this with the negated decrease when they apply
    /// and the decrease itself when they end.
    pub(super) fn shift_multiplier(&mut self, delta: f32) {
        if delta == 0.0 {
            return;
        }
        let sum = self.multiplier + delta;
        self.multiplier = if (sum - NEUTRAL).abs() < SNAP {
            NEUTRAL
        } else {
            sum
        };
    }

    /// Sets the flat bonus `source` grants, replacing its earlier value.
    pub(super) fn set_bonus(&mut self, source: HandleId, value: f32) {
        match self.bonuses.iter_mut().find(|(id, _)| *id == source) {
            Some(entry) => entry.1 = value,
            None => self.bonuses.push((source, value)),
        }
    }

    pub(super) fn remove_bonus(&mut self, source: HandleId) {
        self.bonuses.retain(|(id, _)| *id != source);
    }

    /// The largest bonus, never below zero.
    pub(super) fn bonus(&self) -> f32 {
        self.bonuses.iter().fold(
            0.0,
            |best, (_, value)| if *value > best { *value } else { best },
        )
    }

    /// The speed the unit walks at, or `None` without a base.
    pub(super) fn effective(&self) -> Option<f32> {
        let raw = (self.bonus() + self.base?) * self.multiplier;
        Some(self.bounds.map_or(raw, |bounds| bounds.apply(raw)))
    }

    /// A form change writes the new unit type's base and bounds. The multiplier and the bonuses
    /// stay: they belong to buffs and items, not to the unit type.
    pub(super) fn replace_form(&mut self, form: &Self) {
        self.base = form.base;
        self.bounds = form.bounds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: SpeedBounds = SpeedBounds {
        min: 150.0,
        max: 400.0,
    };

    fn id(token: u64) -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(token).unwrap())
    }

    fn near(actual: Option<f32>, expected: f32) {
        let actual = actual.expect("a speed");
        assert!((actual - expected).abs() < 0.01, "{actual} != {expected}");
    }

    fn speed(base: f32) -> MoveSpeed {
        MoveSpeed::new(Some(base), Some(BOUNDS))
    }

    #[test]
    fn the_bonus_is_added_before_the_multiplier() {
        let mut state = speed(270.0);
        state.set_bonus(id(1), 60.0);
        assert_eq!(state.effective(), Some(330.0));
        state.shift_multiplier(-0.5);
        assert_eq!(state.effective(), Some(165.0));
        // Not (base * multiplier) + bonus, which would be 195.
        state.set_base(300.0);
        assert_eq!(state.effective(), Some(180.0));
    }

    #[test]
    fn only_the_largest_bonus_counts_and_a_negative_one_never_does() {
        let mut state = speed(200.0);
        state.set_bonus(id(1), 20.0);
        state.set_bonus(id(2), 60.0);
        state.set_bonus(id(3), -80.0);
        assert_eq!(state.bonus(), 60.0);
        assert_eq!(state.effective(), Some(260.0));
        state.remove_bonus(id(2));
        assert_eq!(state.effective(), Some(220.0));
        state.set_bonus(id(1), 10.0);
        assert_eq!(state.effective(), Some(210.0));
        state.remove_bonus(id(1));
        state.remove_bonus(id(3));
        assert_eq!(state.effective(), Some(200.0));
    }

    #[test]
    fn multiplier_deltas_add_and_undo_exactly() {
        let mut state = speed(300.0);
        state.shift_multiplier(-0.3);
        state.shift_multiplier(-0.2);
        near(state.effective(), 150.0);
        state.shift_multiplier(0.3);
        near(state.effective(), 240.0);
        state.shift_multiplier(0.2);
        assert_eq!(
            state.multiplier, NEUTRAL,
            "a sum within 0.001 of 1 snaps to 1"
        );
        assert_eq!(state.effective(), Some(300.0));
    }

    #[test]
    fn a_multiplier_at_or_below_zero_leaves_the_lower_bound() {
        let mut state = speed(300.0);
        state.shift_multiplier(-1.0);
        assert_eq!(state.effective(), Some(150.0));
        state.shift_multiplier(-0.4);
        assert_eq!(state.effective(), Some(150.0));
        state.shift_multiplier(1.4);
        assert_eq!(state.effective(), Some(300.0));
    }

    #[test]
    fn the_upper_bound_wins_over_the_lower_one() {
        let inverted = SpeedBounds {
            min: 500.0,
            max: 400.0,
        };
        assert_eq!(
            MoveSpeed::new(Some(100.0), Some(inverted)).effective(),
            Some(400.0)
        );
        let mut fast = speed(380.0);
        fast.set_bonus(id(1), 100.0);
        assert_eq!(fast.effective(), Some(400.0));
    }

    #[test]
    fn without_bounds_or_base_nothing_is_limited_or_invented() {
        let mut unbounded = MoveSpeed::new(Some(10.0), None);
        assert!(!unbounded.bounded());
        unbounded.shift_multiplier(-0.5);
        assert_eq!(unbounded.effective(), Some(5.0));
        let mut immobile = MoveSpeed::new(None, Some(BOUNDS));
        immobile.set_bonus(id(1), 60.0);
        immobile.shift_multiplier(-0.5);
        assert_eq!(immobile.effective(), None);
        immobile.set_base(100.0);
        assert_eq!(
            immobile.effective(),
            Some(150.0),
            "(60 + 100) * 0.5 raised to the minimum"
        );
    }

    #[test]
    fn unit_type_bounds_defer_to_misc_when_zero_and_clamp_otherwise() {
        let misc = (150.0, 400.0);
        assert_eq!(SpeedBounds::resolve(0.0, 0.0, misc), BOUNDS);
        let own = SpeedBounds::resolve(75.0, 522.0, misc);
        assert_eq!((own.min, own.max), (75.0, 522.0));
        let hard = SpeedBounds::resolve(0.25, 9000.0, misc);
        assert_eq!((hard.min, hard.max), (ENGINE_MIN_SPEED, ENGINE_MAX_SPEED));
        let mixed = SpeedBounds::resolve(200.0, 0.0, misc);
        assert_eq!((mixed.min, mixed.max), (200.0, 400.0));
    }

    #[test]
    fn a_form_change_keeps_buffs_and_items_and_takes_the_new_base_and_bounds() {
        let mut state = speed(270.0);
        state.set_bonus(id(1), 60.0);
        state.shift_multiplier(-0.5);
        let form = MoveSpeed::new(
            Some(100.0),
            Some(SpeedBounds {
                min: 75.0,
                max: 522.0,
            }),
        );
        state.replace_form(&form);
        assert_eq!(state.effective(), Some(80.0));
    }
}

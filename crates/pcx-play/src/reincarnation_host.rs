//! The death handler of a held Ankh of Reincarnation (`AIrc`). The decode is in
//! `docs/re-notes/passive-held-items.md`, "`AIrc`".
//!
//! The unit death routine (`0x6884f0`) runs the death handlers of the dying unit's abilities by
//! stage before it posts the death events (`0xd0154`, `0xd0155`). The handler of `AIrc`
//! (`0xb8b870`) fires when the item still has a charge, no revive record of the unit is pending
//! and the unit's position is not unwalkable. It then:
//!
//! 1. empties the two handle fields `[unit+0x45c]` and `[unit+0x460]` (`0x69cf30` with no unit),
//!    whose meaning is not established and which the recreation does not model;
//! 2. spends a charge (`0x6c6f10`, no cooldown): a perishable item that reaches none is destroyed;
//! 3. creates a revive record that brings the unit back `DataA1` seconds later (event `0xd01c4`,
//!    handler `0xb8aeb0`, revive `0xb8ad60`) where it died, with the life and mana of
//!    [`Reincarnation`].
//!
//! The unit is dead in between, exactly as after any death. Refused as not decoded, before the
//! death changes anything: a dying unit that also holds a hero reincarnation ability (`AOre`,
//! `ACrn`, whose handler runs a stage earlier and decides on cooldown and mana), several held
//! ankhs, a flying unit (the walkability test also reads the dynamic spatial entries a flier can
//! hover over), a host without the item-charge or the terrain pathing admission, and a revive
//! whose position the placement refuses.
use super::revive::{Restore, Reviver};
use super::{Entry, Error, Failure, ObjectHost};
use crate::inventory_powerup::Reincarnation;
use pcx_script::HandleId;
use std::collections::BTreeMap;

const NAME: &str = "reincarnation";

struct Pending {
    due: f64,
    point: [f32; 2],
    spec: Reincarnation,
    mana_at_death: f32,
}

#[derive(Default)]
pub(super) struct State {
    pending: BTreeMap<HandleId, Pending>,
    /// Tests answer the walkability query here: a unit-test host has no map raster.
    #[cfg(test)]
    unwalkable: Option<bool>,
}

/// A reincarnation that is checked and not yet applied.
pub(super) struct Plan {
    item: HandleId,
    spec: Reincarnation,
    point: [f32; 2],
    mana_at_death: f32,
}

impl ObjectHost {
    fn reincarnation_error(&self, reason: impl Into<String>) -> Error {
        self.inner.error(NAME, Failure::UnitActivity(reason.into()))
    }

    /// Whether the unit's position is unwalkable terrain, the test that stops the handler.
    fn reincarnation_unwalkable(&self, point: [f32; 2]) -> Result<bool, Error> {
        #[cfg(test)]
        if let Some(answer) = self.reincarnation.unwalkable {
            return Ok(answer);
        }
        self.terrain_unwalkable(point)
    }

    /// The reincarnation `unit` would make if it died now, checked without any change. `None`
    /// when no held ankh fires: none held, no charge left, a revive already pending, or an
    /// unwalkable position.
    pub(super) fn plan_reincarnation(&self, unit: HandleId) -> Result<Option<Plan>, Error> {
        let mut held = Vec::new();
        for (item, record) in self.executed_items() {
            if record.held_by.is_some_and(|(owner, _)| owner == unit)
                && let Some(spec) = self.held_attributes(item)?.reincarnation
            {
                held.push((item, spec));
            }
        }
        let Some(&(item, spec)) = held.first() else {
            return Ok(None);
        };
        if held.len() > 1 {
            return Err(self.reincarnation_error(
                "a unit that holds several ankhs dies: which one fires first follows the order of \
                 its ability list, which is not decoded",
            ));
        }
        let record = self.executed_unit(unit)?;
        if let Some(abilities) = &record.abilities {
            for (alias, rank) in abilities.levels().filter(|(_, rank)| *rank > 0) {
                let parent = self
                    .ability_parent_code(alias)
                    .map_err(|why| self.reincarnation_error(why))?;
                let code = self
                    .ability_data_text(parent, "code")
                    .map_err(|why| self.reincarnation_error(why))?;
                if self.hero_reincarnation_class(code.trim()) {
                    return Err(self.reincarnation_error(format!(
                        "a unit with ability {} (rank {rank}, class {}) dies holding an ankh: the \
                         hero handler runs a stage earlier and decides on cooldown and mana, which \
                         the recreation does not model",
                        String::from_utf8_lossy(&alias),
                        code.trim()
                    )));
                }
            }
        }
        if self.item_use.is_none() {
            return Err(self.reincarnation_error(
                "an ankh spends a charge when its holder dies: the item-charge host is not admitted",
            ));
        }
        if self.cached_item_charges(item)? <= 0 || self.reincarnation.pending.contains_key(&unit) {
            return Ok(None);
        }
        if super::unit_types::flying(&record.definition) {
            return Err(self.reincarnation_error(
                "a flying unit dies holding an ankh: the walkability test of the handler also reads \
                 the dynamic spatial entries, which are not decoded",
            ));
        }
        let (Some(point), Some(vitals)) = (record.position, record.vitals.as_ref()) else {
            return Err(self.reincarnation_error("the dying unit has no position or vitals"));
        };
        if self.reincarnation_unwalkable(point)? {
            return Ok(None);
        }
        Ok(Some(Plan {
            item,
            spec,
            point,
            mana_at_death: vitals.mana,
        }))
    }

    /// Applies a checked plan: the charge is spent, the perishable item goes, and the revive is
    /// scheduled `DataA1` seconds from now.
    pub(super) fn commit_reincarnation(&mut self, unit: HandleId, plan: Plan) -> Result<(), Error> {
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.reincarnation_error("clock missing"))?;
        let spent = self.spend_item_charge(plan.item)?;
        if spent.remove {
            *self.inner.entry_mut(plan.item)? = Entry::RemovedItem;
        }
        self.reincarnation.pending.insert(
            unit,
            Pending {
                due: now + f64::from(plan.spec.delay),
                point: plan.point,
                spec: plan.spec,
                mana_at_death: plan.mana_at_death,
            },
        );
        Ok(())
    }

    /// Revives every unit whose record is due at `now`. A unit that was removed or revived by
    /// other means in the meantime keeps no record; a revive the placement refuses is an error.
    pub(super) fn advance_reincarnations(&mut self, now: f64) -> Result<(), Error> {
        let due: Vec<HandleId> = self
            .reincarnation
            .pending
            .iter()
            .filter(|(_, pending)| pending.due <= now)
            .map(|(unit, _)| *unit)
            .collect();
        for unit in due {
            let Some(pending) = self.reincarnation.pending.remove(&unit) else {
                continue;
            };
            let dead = self
                .executed_unit(unit)
                .ok()
                .is_some_and(|record| record.combat.as_ref().is_some_and(|c| c.dead));
            if !dead {
                continue;
            }
            let restore = Restore::Reincarnation {
                spec: pending.spec,
                mana_at_death: pending.mana_at_death,
            };
            if !self.revive_unit(NAME, unit, pending.point, restore, Reviver::Handler)? {
                return Err(self.reincarnation_error(format!(
                    "the revive at {:?} is refused by the placement: the client puts the unit \
                     back without a footprint test, which is not decoded",
                    pending.point
                )));
            }
        }
        Ok(())
    }

    /// A removed unit keeps no revive record.
    pub(super) fn forget_reincarnation_unit(&mut self, unit: HandleId) {
        self.reincarnation.pending.remove(&unit);
    }

    /// Whether a revive record of `unit` is pending.
    #[cfg(test)]
    pub(crate) fn reincarnation_pending(&self, unit: HandleId) -> bool {
        self.reincarnation.pending.contains_key(&unit)
    }

    /// Answers the walkability query of the handler for tests.
    #[cfg(test)]
    pub(crate) fn test_set_unwalkable(&mut self, answer: bool) {
        self.reincarnation.unwalkable = Some(answer);
    }
}

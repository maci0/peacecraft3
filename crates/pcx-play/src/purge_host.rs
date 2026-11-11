//! The Purge cast (`Aprg`) an orb makes on an armed hit: the lightning orb `AIlb` through its
//! sibling `AIlp`, and the spell orb `AIsb` through the wrapped `AIpg`. The decode is in
//! `docs/re-notes/passive-held-items.md`, "Purge".
//!
//! One cast (`0xb9e350`) does, in order:
//!
//! 1. For a summoned enemy it applies the row's `DataC` as damage of the magic type; otherwise
//!    it applies a zero-damage attack record (`0xba0870`). Only the zero-damage record is
//!    modelled as nothing; the magic-type damage path of the target (`0x6677c0`) is not
//!    decoded, so a summoned enemy refuses when `DataC` is not zero.
//! 2. It dispels the target (`0x6d87a0`): every positive or negative buff whose class is magic,
//!    except timed-life buffs and auras.
//! 3. On a living target the caster's owner is not allied with, it replaces any `Bprg` with a
//!    new one (`0xba01b0`, `0xb9d5f0`): the movement multiplier loses [`FULL_SLOW`] and gets it
//!    back in `DataA` equal steps, one every `Dur / DataA` seconds. The buff ends `Dur` plus the
//!    image's duration slack after the cast and gives back what is left.
//!
//! Refused as not decoded: the movement pause of `DataD` and `DataE` (the movement object's
//! disable counter, `0xb666b0`), a dispel that meets a buff whose magic bit nobody read, and a
//! polymorphed target (the speed writes of the type change).
use super::unit_buff::{ActiveBuff, BuffTraits, PURGE_DISPEL};
use super::{Error, Failure, ObjectHost};
use crate::inventory_powerup::{Purge, purge::FULL_SLOW};
use pcx_script::HandleId;
use std::collections::BTreeMap;

const NAME: &str = "purge";
/// A remaining slow smaller than this ends the decay (`0x10db054`, `0xba1185`).
const DECAY_DONE: f32 = 0.001;

/// The `Bprg` buff on one unit.
struct ActiveSlow {
    buff: [u8; 4],
    /// What the buff still removes from the movement multiplier (`+0xe0`).
    magnitude: f32,
    /// `None` when the row has no frequency: the buff slows nothing.
    decay: Option<Decay>,
    expires_at: f64,
    /// The unit had a speed host when the slow applied.
    hosted: bool,
}

struct Decay {
    step: f32,
    interval: f64,
    next_step: f64,
}

#[derive(Default)]
pub(super) struct State {
    active: BTreeMap<HandleId, ActiveSlow>,
}

impl ObjectHost {
    fn purge_error(&self, reason: impl Into<String>) -> Error {
        self.inner.error(NAME, Failure::UnitActivity(reason.into()))
    }

    /// Whether a Purge buff owns part of the unit's move speed.
    pub(crate) fn unit_purge_slowed(&self, unit: HandleId) -> bool {
        self.purges
            .active
            .get(&unit)
            .is_some_and(|slow| slow.hosted)
    }

    /// Whether `target` is a legal target of the Purge: its class list names the unit's movement
    /// class, and a structure only when it names structures.
    pub(super) fn purge_target_valid(
        &self,
        purge: &Purge,
        target: HandleId,
    ) -> Result<bool, Error> {
        let Some(unit) = self.live(target) else {
            return Ok(false);
        };
        let structure =
            super::unit_types::structure(&unit.definition).map_err(|e| self.purge_error(e))?;
        let class = if super::unit_types::flying(&unit.definition) {
            purge.targets.air
        } else {
            purge.targets.ground
        };
        Ok(class && (purge.targets.structure || !structure))
    }

    /// One cast on `target` by `caster`. Every refusal is raised before the first change.
    pub(super) fn cast_purge(
        &mut self,
        purge: &Purge,
        caster: HandleId,
        target: HandleId,
    ) -> Result<(), Error> {
        let Some(victim) = self.live(target) else {
            return Ok(());
        };
        let hero = victim.definition.rawcode[0].is_ascii_uppercase();
        let summoned = victim.summoned;
        // `0x696780`: the target's owner is not allied with the caster's owner. `0x696a90`: the
        // caster's owner is allied with the target's owner.
        let enemy = !self.is_ally(target, caster);
        let allied = self.is_ally(caster, target);
        if enemy && summoned && purge.summoned_damage != 0.0 {
            return Err(self.purge_error(format!(
                "a Purge cast on a summoned enemy deals {} magic-type damage (record flag \
                 0x4000) through the unit damage path (0x6677c0), which is not decoded",
                purge.summoned_damage
            )));
        }
        let pause = if hero {
            purge.hero_pause
        } else {
            purge.unit_pause
        };
        if !allied && pause != 0.0 {
            return Err(self.purge_error(format!(
                "a Purge cast pauses the target's movement for {pause} s (DataD, DataE), which \
                 is not hosted: the movement object's disable counter (0xb666b0) is not decoded"
            )));
        }
        if let Some(buff) = self.undecoded_magic_buff(target) {
            return Err(self.purge_error(format!(
                "the dispel of a Purge cast tests the magic bit of buff {}, which the class \
                 reads (slot 0x1d8) and nobody has decoded",
                String::from_utf8_lossy(&buff)
            )));
        }
        if !allied && purge.slow.is_some() && self.unit_polymorphed(target) {
            return Err(self.purge_error(
                "a Purge slow on a polymorphed unit is not modelled: the speed writes of the \
                 polymorph type change are not decoded",
            ));
        }
        self.remove_matching_buffs(target, &PURGE_DISPEL)?;
        if allied {
            return Ok(());
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.purge_error("clock missing"))?;
        self.end_purge_slow(target)?;
        self.attach_unit_buff(
            target,
            ActiveBuff::new(
                purge.buff,
                BuffTraits {
                    negative: true,
                    // `Bprg` slot `+0x1d8` (`0xb9da00`) returns 0, so a later Purge keeps it.
                    magic_decoded: true,
                    ..BuffTraits::default()
                },
            ),
        )?;
        let decay = purge.slow.map(|slow| {
            let interval = f64::from(if hero {
                slow.hero_interval
            } else {
                slow.interval
            });
            Decay {
                step: slow.step,
                interval,
                next_step: now + interval,
            }
        });
        let hosted = if decay.is_some() {
            match self.shift_unit_speed(target, -FULL_SLOW, NAME) {
                Ok(hosted) => hosted,
                Err(error) => {
                    self.detach_unit_buff(target, purge.buff);
                    return Err(error);
                }
            }
        } else {
            false
        };
        let duration = if hero {
            purge.hero_duration
        } else {
            purge.duration
        };
        self.purges.active.insert(
            target,
            ActiveSlow {
                buff: purge.buff,
                magnitude: FULL_SLOW,
                decay,
                expires_at: now + f64::from(duration),
                hosted,
            },
        );
        Ok(())
    }

    /// Runs the decay steps and the expiries due at `now`, in time order. A step that falls on
    /// the expiry runs first: the duration slack puts the expiry after the last step.
    pub(super) fn advance_purges(&mut self, now: f64) -> Result<(), Error> {
        let units: Vec<HandleId> = self.purges.active.keys().copied().collect();
        for unit in units {
            while let Some(slow) = self.purges.active.get_mut(&unit) {
                let step_due = slow
                    .decay
                    .as_ref()
                    .is_some_and(|d| d.next_step <= now && d.next_step <= slow.expires_at);
                if step_due {
                    let decay = slow.decay.as_mut().expect("checked above");
                    decay.next_step += decay.interval;
                    let (step, magnitude, hosted) = (decay.step, slow.magnitude, slow.hosted);
                    if magnitude.abs() < DECAY_DONE {
                        continue;
                    }
                    let remaining = magnitude - step;
                    slow.magnitude = remaining;
                    if hosted {
                        self.shift_unit_speed(unit, magnitude - remaining, NAME)?;
                    }
                } else if slow.expires_at <= now {
                    self.end_purge_slow(unit)?;
                    break;
                } else {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Gives back what the slow still removes and drops the buff.
    fn end_purge_slow(&mut self, unit: HandleId) -> Result<(), Error> {
        let Some(slow) = self.purges.active.remove(&unit) else {
            return Ok(());
        };
        if slow.hosted {
            self.shift_unit_speed(unit, slow.magnitude, NAME)?;
        }
        self.detach_unit_buff(unit, slow.buff);
        Ok(())
    }

    /// A buff removal (`UnitRemoveBuffs`) of `rawcode` ends the slow it carries.
    pub(super) fn purge_buff_removed(
        &mut self,
        unit: HandleId,
        rawcode: [u8; 4],
    ) -> Result<(), Error> {
        if self
            .purges
            .active
            .get(&unit)
            .is_some_and(|slow| slow.buff == rawcode)
        {
            self.end_purge_slow(unit)?;
        }
        Ok(())
    }

    /// A cast without a hit, for tests that need a relation or a unit kind a swing cannot reach.
    #[cfg(test)]
    pub(crate) fn test_cast_purge(
        &mut self,
        purge: &Purge,
        caster: HandleId,
        target: HandleId,
    ) -> Result<(), Error> {
        self.cast_purge(purge, caster, target)
    }

    /// Marks `unit` as a summoned unit.
    #[cfg(test)]
    pub(crate) fn test_mark_summoned(&mut self, unit: HandleId) {
        if let super::Entry::Unit { record } = self.inner.entry_mut(unit).expect("a unit") {
            record.summoned = true;
        }
    }

    /// A dead or removed unit keeps no slow.
    pub(super) fn forget_purge_unit(&mut self, unit: HandleId) -> Result<(), Error> {
        self.end_purge_slow(unit)
    }
}

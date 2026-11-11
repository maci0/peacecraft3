//! Held aura items (`AIau` Legion Doom-Horn, `AIad` Devotion Aura, `AIba` Brilliance Aura). The decoded rules and their
//! evidence are in `docs/re-notes/passive-held-items.md`, "Held auras".
//!
//! While an item is held its aura source runs two clocks from the pickup:
//!
//! - a timer every `tick` seconds (2.0) that arms the next scan to report units that stay in range
//!   and applies the buff to the holder;
//! - a range scan every `scan` seconds (0.5) over `Area1` around the holder. A unit that was not
//!   in the previous scan's list is reported at once; a listed unit is reported only on the scan
//!   after a timer tick. Leaving the range reports nothing.
//!
//! A report applies the buff to the unit when it passes the target test. The buff is one per unit
//! and buff id, so two sources of one class share it. It ends `duration` seconds (4.0) after its
//! last application or refresh, with no leave event and no link to its source: dropping the item
//! or losing the holder stops the source, and the buffs run out on their own timers.
//!
//! A refresh restarts the timer unless the stored values are all larger than the new ones by more
//! than the tolerance, and raises each stored value that the new one exceeds by more than the
//! tolerance; a stored value never falls before the buff ends.
//!
//! Open, not modelled: the unit-state flags of the target test beyond death, polymorph and
//! illusion (burrowed, animated and invisible holders, `[unit+0x164] & 0x10`), the body of the
//! allowed-target test `0x6adb90` (the shipped `targs1` tokens are read by name), the target-side
//! veto chain `0x682180` (no recreated state has an ability that vetoes), and the armor shown in
//! the unit panel.
use super::unit_buff::{ActiveBuff, BuffTraits};
use super::{Error, Failure, ObjectHost};
use crate::inventory_powerup::{AuraEffect, HeldAura};
use pcx_script::HandleId;
use pcx_sim::world::Definition;
use std::collections::{BTreeMap, BTreeSet};

const NAME: &str = "aura";

/// The buff class bits of both aura buffs: positive (`0x1e4` returns 1), not negative (`0x1e0`
/// returns 0) and an aura (`0x1e8` returns 1), so a dispel leaves them alone.
const BUFF_TRAITS: BuffTraits = BuffTraits {
    positive: true,
    negative: false,
    magic: false,
    physical: false,
    timed_life: false,
    aura: true,
    auto_dispellable: false,
    magic_decoded: false,
};

/// One held item's running aura.
struct Source {
    holder: HandleId,
    aura: HeldAura,
    next_tick: f64,
    next_scan: f64,
    /// The units the last scan found in range.
    listed: BTreeSet<HandleId>,
    /// A timer tick since the last scan: the next scan reports listed units too.
    report_stay: bool,
}

/// The aura buff on one unit.
struct AppliedBuff {
    effect: AuraEffect,
    expires_at: f64,
    /// The amount the buff added to the unit's speed multiplier, `None` when it added none.
    speed: Option<f32>,
    tolerance: f32,
}

#[derive(Default)]
pub(super) struct State {
    sources: BTreeMap<HandleId, Source>,
    buffs: BTreeMap<(HandleId, [u8; 4]), AppliedBuff>,
}

/// Which event of the clocks runs next.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Due {
    Tick(f64, HandleId),
    Scan(f64, HandleId),
    Expiry(f64, HandleId, [u8; 4]),
}

impl Due {
    fn time(self) -> f64 {
        match self {
            Due::Tick(at, _) | Due::Scan(at, _) | Due::Expiry(at, ..) => at,
        }
    }

    /// Ties run the timer tick first (so the scan sees its stay report), then the scan, then the
    /// expiries.
    fn rank(self) -> u8 {
        match self {
            Due::Tick(..) => 0,
            Due::Scan(..) => 1,
            Due::Expiry(..) => 2,
        }
    }
}

/// `new` is smaller than `stored` by at least the tolerance.
fn exceeds(stored: f32, new: f32, tolerance: f32) -> bool {
    stored > new && (stored - new).abs() >= tolerance
}

impl ObjectHost {
    fn aura_error(&self, reason: impl Into<String>) -> Error {
        self.inner.error(NAME, Failure::UnitActivity(reason.into()))
    }

    /// Refuses before any write when the holder cannot host an aura source: it needs the game
    /// clock and a position to scan around.
    pub(super) fn require_aura_host(&self, holder: HandleId) -> Result<(), Error> {
        if self.timer_game_time().is_none() {
            return Err(self.aura_error("an aura needs the game clock"));
        }
        if self.executed_unit(holder)?.position.is_none() {
            return Err(self.aura_error("an aura needs the holder's position"));
        }
        Ok(())
    }

    /// Starts the aura of `item` held by `holder`: the timers run from now and the holder gets
    /// the buff at once (slot `0xb8fd10` applies it to the owner when the ability is active).
    pub(super) fn start_item_aura(
        &mut self,
        holder: HandleId,
        item: HandleId,
        aura: HeldAura,
    ) -> Result<(), Error> {
        self.require_aura_host(holder)?;
        let now = self.timer_game_time().expect("checked clock");
        self.auras.sources.insert(
            item,
            Source {
                holder,
                aura,
                next_tick: now + aura.timing.tick,
                next_scan: now + aura.timing.scan,
                listed: BTreeSet::new(),
                report_stay: false,
            },
        );
        self.aura_report(item, holder, now)
    }

    /// Stops the aura of `item`: its timers and scan end. Buffs it applied are not removed.
    pub(super) fn stop_item_aura(&mut self, item: HandleId) {
        self.auras.sources.remove(&item);
    }

    fn aura_active(&self, holder: HandleId) -> bool {
        self.executed_unit(holder)
            .is_ok_and(|unit| !unit.combat.as_ref().is_some_and(|combat| combat.dead))
    }

    /// Whether the unit class of `definition` is listed by the aura row.
    fn aura_covers(&self, aura: &HeldAura, definition: &Definition) -> Result<bool, Error> {
        let structure = super::unit_types::structure(definition).map_err(|e| self.aura_error(e))?;
        Ok(if structure {
            aura.targets.structure
        } else if super::unit_types::flying(definition) {
            aura.targets.air
        } else {
            aura.targets.ground
        })
    }

    /// The holder side of the target test (`0xb90c10`): the ability is active (the holder lives)
    /// and the holder is not polymorphed (`Misc/PolymorphedUnitsBestowAuras` is zero).
    fn aura_holder_ok(&self, holder: HandleId) -> bool {
        self.aura_active(holder) && !self.unit_polymorphed(holder)
    }

    /// The target side (`0xb90440`) for the rows decoded: the unit is alive, of a listed class
    /// and the holder or an ally of the holder; an illusion holder bestows nothing on others
    /// (`Misc/IllusionsBestowAuras` is zero).
    fn aura_eligible(
        &self,
        holder: HandleId,
        aura: &HeldAura,
        target: HandleId,
    ) -> Result<bool, Error> {
        let Ok(unit) = self.executed_unit(target) else {
            return Ok(false);
        };
        if unit.combat.as_ref().is_some_and(|combat| combat.dead) {
            return Ok(false);
        }
        if target != holder
            && (self.executed_unit(holder)?.illusion.is_some() || !self.is_ally(holder, target))
        {
            return Ok(false);
        }
        self.aura_covers(aura, &unit.definition)
    }

    /// One report of `target` to the source of `item`: applies or refreshes the buff.
    fn aura_report(&mut self, item: HandleId, target: HandleId, now: f64) -> Result<(), Error> {
        let Some(source) = self.auras.sources.get(&item) else {
            return Ok(());
        };
        let (holder, aura) = (source.holder, source.aura);
        if !self.aura_holder_ok(holder) || !self.aura_eligible(holder, &aura, target)? {
            return Ok(());
        }
        let key = (target, aura.buff);
        let expires_at = now + aura.timing.duration;
        let Some(stored) = self.auras.buffs.get(&key) else {
            return self.aura_create(target, &aura, expires_at, now);
        };
        let tolerance = stored.tolerance;
        match (stored.effect, aura.effect) {
            (
                AuraEffect::Unholy {
                    speed: stored_speed,
                    regen: stored_regen,
                },
                AuraEffect::Unholy { speed, regen },
            ) => {
                if exceeds(stored_speed, speed, tolerance)
                    && exceeds(stored_regen, regen, tolerance)
                {
                    return Ok(());
                }
                self.aura_restart(&key, expires_at);
                let raise = |stored: f32, new: f32| new > stored && (new - stored) >= tolerance;
                if raise(stored_speed, speed) || raise(stored_regen, regen) {
                    self.aura_replace(
                        target,
                        &key,
                        AuraEffect::Unholy {
                            speed: if raise(stored_speed, speed) {
                                speed
                            } else {
                                stored_speed
                            },
                            regen: if raise(stored_regen, regen) {
                                regen
                            } else {
                                stored_regen
                            },
                        },
                        now,
                    )?;
                }
            }
            (stored, new) if std::mem::discriminant(&stored) == std::mem::discriminant(&new) => {
                let (Some(stored_amount), Some(amount)) = (stored.amount(), new.amount()) else {
                    return Ok(());
                };
                if exceeds(stored_amount, amount, tolerance) {
                    return Ok(());
                }
                self.aura_restart(&key, expires_at);
                if amount > stored_amount && (amount - stored_amount) >= tolerance {
                    self.aura_replace(target, &key, new, now)?;
                }
            }
            _ => return Err(self.aura_error("one buff id names two aura classes")),
        }
        Ok(())
    }

    fn aura_restart(&mut self, key: &(HandleId, [u8; 4]), expires_at: f64) {
        if let Some(buff) = self.auras.buffs.get_mut(key) {
            buff.expires_at = expires_at;
        }
    }

    /// Integrates the unit's regeneration to `now` before an aura changes its rate.
    fn aura_settle_regen(&mut self, unit: HandleId, now: f64) -> Result<(), Error> {
        let living = self.executed_unit(unit).is_ok_and(|record| {
            record.vitals.is_some() && !record.combat.as_ref().is_some_and(|c| c.dead)
        });
        if living {
            self.advance_unit_regen(now)?;
        }
        Ok(())
    }

    /// Adds the effect of `effect` to `unit` and returns the speed amount it added.
    fn aura_apply_effect(
        &mut self,
        unit: HandleId,
        effect: AuraEffect,
        now: f64,
    ) -> Result<Option<f32>, Error> {
        if self.executed_unit(unit)?.illusion.is_some() {
            return Ok(None);
        }
        if effect.changes_regen() {
            self.aura_settle_regen(unit, now)?;
        }
        let AuraEffect::Unholy { speed, .. } = effect else {
            return Ok(None);
        };
        if speed == 0.0 {
            return Ok(None);
        }
        if self.unit_polymorphed(unit) {
            return Err(self.aura_error(
                "an aura speed on a polymorphed unit is not modelled: the speed writes of the polymorph type change are not decoded",
            ));
        }
        Ok(self.shift_unit_speed(unit, speed, NAME)?.then_some(speed))
    }

    fn aura_create(
        &mut self,
        target: HandleId,
        aura: &HeldAura,
        expires_at: f64,
        now: f64,
    ) -> Result<(), Error> {
        self.attach_unit_buff(target, ActiveBuff::new(aura.buff, BUFF_TRAITS))?;
        let speed = match self.aura_apply_effect(target, aura.effect, now) {
            Ok(speed) => speed,
            Err(error) => {
                self.detach_unit_buff(target, aura.buff);
                return Err(error);
            }
        };
        self.auras.buffs.insert(
            (target, aura.buff),
            AppliedBuff {
                effect: aura.effect,
                expires_at,
                speed,
                tolerance: aura.timing.tolerance,
            },
        );
        Ok(())
    }

    /// Takes the old effect off the unit and puts `effect` on, keeping the buff and its timer.
    fn aura_replace(
        &mut self,
        unit: HandleId,
        key: &(HandleId, [u8; 4]),
        effect: AuraEffect,
        now: f64,
    ) -> Result<(), Error> {
        self.aura_remove_effect(unit, key, now)?;
        let speed = self.aura_apply_effect(unit, effect, now)?;
        if let Some(buff) = self.auras.buffs.get_mut(key) {
            buff.effect = effect;
            buff.speed = speed;
        }
        Ok(())
    }

    /// Takes the speed amount the buff added back out and settles the regeneration the buff's
    /// rate has accrued; the stored effect stays until the caller replaces or drops it.
    fn aura_remove_effect(
        &mut self,
        unit: HandleId,
        key: &(HandleId, [u8; 4]),
        now: f64,
    ) -> Result<(), Error> {
        let Some(buff) = self.auras.buffs.get(key) else {
            return Ok(());
        };
        let (effect, speed) = (buff.effect, buff.speed);
        if effect.changes_regen() {
            self.aura_settle_regen(unit, now)?;
        }
        if let Some(speed) = speed {
            self.shift_unit_speed(unit, -speed, NAME)?;
            if let Some(buff) = self.auras.buffs.get_mut(key) {
                buff.speed = None;
            }
        }
        Ok(())
    }

    /// Ends the buff: its effect comes off the unit and the buff leaves the unit's list.
    fn aura_end(&mut self, unit: HandleId, buff: [u8; 4], now: f64) -> Result<(), Error> {
        let key = (unit, buff);
        self.aura_remove_effect(unit, &key, now)?;
        self.auras.buffs.remove(&key);
        self.detach_unit_buff(unit, buff);
        Ok(())
    }

    /// The next due event of the clocks, or `None` when nothing is due by `now`.
    fn aura_next_due(&self, now: f64) -> Option<Due> {
        let sources = self.auras.sources.iter().flat_map(|(item, source)| {
            [
                Due::Tick(source.next_tick, *item),
                Due::Scan(source.next_scan, *item),
            ]
        });
        let expiries = self
            .auras
            .buffs
            .iter()
            .map(|((unit, buff), applied)| Due::Expiry(applied.expires_at, *unit, *buff));
        sources
            .chain(expiries)
            .filter(|due| due.time() <= now)
            .min_by(|a, b| {
                a.time()
                    .total_cmp(&b.time())
                    .then(a.rank().cmp(&b.rank()))
                    .then_with(|| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            })
    }

    /// Runs every tick, scan and expiry due by `now`, in time order. Reports apply at `now`, the
    /// clocks keep their own cadence.
    pub(super) fn advance_auras(&mut self, now: f64) -> Result<(), Error> {
        while let Some(due) = self.aura_next_due(now) {
            match due {
                Due::Tick(_, item) => self.aura_tick(item, now)?,
                Due::Scan(_, item) => self.aura_scan(item, now)?,
                Due::Expiry(_, unit, buff) => self.aura_end(unit, buff, now)?,
            }
        }
        Ok(())
    }

    fn aura_tick(&mut self, item: HandleId, now: f64) -> Result<(), Error> {
        let Some(source) = self.auras.sources.get_mut(&item) else {
            return Ok(());
        };
        source.next_tick += source.aura.timing.tick;
        source.report_stay = true;
        let holder = source.holder;
        self.aura_report(item, holder, now)
    }

    fn aura_scan(&mut self, item: HandleId, now: f64) -> Result<(), Error> {
        let Some(source) = self.auras.sources.get_mut(&item) else {
            return Ok(());
        };
        source.next_scan += source.aura.timing.scan;
        let stay = std::mem::take(&mut source.report_stay);
        let (holder, aura) = (source.holder, source.aura);
        let in_range = self.aura_in_range(holder, &aura)?;
        let Some(source) = self.auras.sources.get_mut(&item) else {
            return Ok(());
        };
        let reported: Vec<HandleId> = in_range
            .iter()
            .copied()
            .filter(|unit| stay || !source.listed.contains(unit))
            .collect();
        source.listed = in_range;
        for unit in reported {
            self.aura_report(item, unit, now)?;
        }
        Ok(())
    }

    /// The eligible units within `Area1` of the holder, counting the collision radius of the unit
    /// (the same reach the range event and the orb splash use). The holder is one of them.
    fn aura_in_range(
        &self,
        holder: HandleId,
        aura: &HeldAura,
    ) -> Result<BTreeSet<HandleId>, Error> {
        let Some(center) = self.executed_unit(holder)?.position else {
            return Ok(BTreeSet::new());
        };
        let mut found = BTreeSet::new();
        for (id, unit) in self.executed_units() {
            let Some(position) = unit.position else {
                continue;
            };
            if !self.aura_eligible(holder, aura, id)? {
                continue;
            }
            let reach = aura.area + self.unit_collision_radius(id, NAME)?;
            if super::combat::distance(position, center) <= reach {
                found.insert(id);
            }
        }
        Ok(found)
    }

    /// Armor the aura buffs add to `unit`: the sum of its devotion buffs.
    pub(crate) fn aura_armor_bonus(&self, unit: HandleId) -> f32 {
        self.aura_buffs_on(unit)
            .map(|buff| match buff.effect {
                AuraEffect::Devotion { armor } => armor,
                AuraEffect::Unholy { .. } | AuraEffect::Brilliance { .. } => 0.0,
            })
            .sum()
    }

    /// Mana points per second the aura buffs add to `unit`.
    pub(crate) fn aura_mana_regen(&self, unit: HandleId) -> f64 {
        self.aura_buffs_on(unit)
            .map(|buff| match buff.effect {
                AuraEffect::Brilliance { mana } => f64::from(mana),
                AuraEffect::Unholy { .. } | AuraEffect::Devotion { .. } => 0.0,
            })
            .sum()
    }

    /// Life points per second the aura buffs add to `unit`.
    pub(crate) fn aura_life_regen(&self, unit: HandleId) -> f64 {
        self.aura_buffs_on(unit)
            .map(|buff| match buff.effect {
                AuraEffect::Unholy { regen, .. } => f64::from(regen),
                AuraEffect::Devotion { .. } | AuraEffect::Brilliance { .. } => 0.0,
            })
            .sum()
    }

    /// The buffs whose effects count on `unit`: none on an illusion, which keeps the buff but
    /// gets no armor, speed or life from it (`Misc/IllusionsGet*`, all zero).
    fn aura_buffs_on(&self, unit: HandleId) -> impl Iterator<Item = &AppliedBuff> {
        let counts = self
            .executed_unit(unit)
            .is_ok_and(|record| record.illusion.is_none());
        self.auras
            .buffs
            .range((unit, [0; 4])..=(unit, [u8::MAX; 4]))
            .filter(move |_| counts)
            .map(|(_, buff)| buff)
    }

    /// A buff removal (`UnitRemoveBuffs`, a dispel) ends the aura buff it names.
    pub(super) fn aura_buff_removed(
        &mut self,
        unit: HandleId,
        rawcode: [u8; 4],
    ) -> Result<(), Error> {
        if !self.auras.buffs.contains_key(&(unit, rawcode)) {
            return Ok(());
        }
        let now = self.timer_game_time().unwrap_or(0.0);
        let key = (unit, rawcode);
        self.aura_remove_effect(unit, &key, now)?;
        self.auras.buffs.remove(&key);
        Ok(())
    }

    /// A dead or removed unit keeps no aura buff.
    pub(super) fn forget_aura_unit(&mut self, unit: HandleId) -> Result<(), Error> {
        let now = self.timer_game_time().unwrap_or(0.0);
        let buffs: Vec<[u8; 4]> = self
            .auras
            .buffs
            .range((unit, [0; 4])..=(unit, [u8::MAX; 4]))
            .map(|((_, buff), _)| *buff)
            .collect();
        for buff in buffs {
            self.aura_end(unit, buff, now)?;
        }
        Ok(())
    }
}

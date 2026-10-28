//! The `attackground` point order: the caster walks into weapon range of a ground point,
//! shoots at it, and the shot damages units around the impact.
//!
//! Established from the selected tables and map edits: the order name resolves through the
//! selected order registry; range, cooldown, damage point, dice, attack type, missile speed
//! and `minRange` come from the unit's `UnitWeapons` and profile fields; the three splash
//! radii (`Farea1`, `Harea1`, `Qarea1`), the two ring factors (`Hfact1`, `Qfact1`) and the
//! unit classes of `splashTargs1` come from the same row. A unit without all five splash
//! numbers refuses the order.
//!
//! Recreation choices, not established against the original: the radii are measured from
//! the impact point to each victim's center; a victim at or inside `Farea1` takes the full
//! damage, inside `Harea1` the `Hfact1` share, inside `Qarea1` the `Qfact1` share; the dice
//! are rolled once per shot and each victim applies its own armor; reach is weapon range
//! plus the caster's collision radius; the projectile lands after distance over missile
//! speed with no arc; the caster keeps firing every cooldown until another order replaces
//! the order.
//!
//! Refused loudly: any `splashTargs1` token other than the unit classes, `enemy`, `notself` and
//! the inert object classes (see [`SplashTargets`](super::combat::SplashTargets)); a target
//! inside `minRange` (the original's reaction is not established). Relationship tokens follow
//! the stock table: a row without `enemy` (the mortar, catapult and meat wagon rows) hits every
//! unit of a listed class in the area, allies and the shooter included, unless `notself`.
//! This is a reading of the table, not an observed game.
//! Destructables, trees and items in the area are not damaged.
use super::combat::{Splash, Weapon, distance};
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

/// A shot in flight; the dice are already rolled.
pub(super) struct GroundMissile {
    source: HandleId,
    point: [f32; 2],
    impact_at: f64,
    base: f32,
    weapon: Weapon,
}

const NAME: &str = "attack ground";
const ATTACK_SEQUENCE: &str = "attack";

impl ObjectHost {
    pub(super) fn is_attack_ground_order(&self, order: i32) -> bool {
        self.combat
            .as_ref()
            .is_some_and(|rules| rules.ground_order == Some(order))
    }

    fn ground_fail(&self, reason: impl Into<String>) -> Error {
        self.inner.error(NAME, Failure::UnitActivity(reason.into()))
    }

    /// `IssuePointOrder`-family call whose order is `attackground`.
    pub(super) fn attack_ground_native(
        &mut self,
        name: &str,
        args: &[Value],
        location: bool,
    ) -> Result<Value, Error> {
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let unit = match args[0] {
            Value::Null => return Ok(Value::Bool(false)),
            Value::Handle(id) => id,
            _ => return Err(fail("unit handle required")),
        };
        if matches!(
            self.inner
                .entry(unit)
                .map_err(|e| self.inner.error(name, e.failure))?,
            Entry::RemovedUnit { .. }
        ) {
            return Ok(Value::Bool(false));
        }
        let record = self.executed_unit(unit)?;
        let Some(combat) = record.combat.as_ref() else {
            return Err(fail("attack ground needs combat state"));
        };
        if record.paused_since.is_some() || combat.dead {
            return Ok(Value::Bool(false));
        }
        let Some(weapon) = combat.weapon else {
            return Err(fail("attack ground needs a weapon"));
        };
        let Some(splash) = weapon.splash else {
            return Err(fail("attack ground needs the weapon's splash columns"));
        };
        if splash.targets.unsupported {
            return Err(fail(
                "splashTargs1 holds a token with no established meaning",
            ));
        }
        let [x, y] = if location {
            self.location_point(&args[2], name)?
        } else {
            let [_, _, Value::Real(x), Value::Real(y)] = args else {
                return Err(self.inner.error(name, Failure::Signature));
            };
            [*x, *y]
        };
        if !x.is_finite() || !y.is_finite() {
            return Err(fail("finite destination required"));
        }
        self.require_morph_interruption(unit, name)?;
        self.interrupt_unit_construction(unit)?;
        self.cancel_inventory_order(unit);
        self.unit_ordered(unit);
        self.clear_combat_target(unit);
        self.steer_unit(unit, None, name)?;
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            record.combat.as_mut().expect("checked above").ground_target = Some([x, y]);
        }
        Ok(Value::Bool(true))
    }

    /// One combat step of a unit with an accepted attack-ground order.
    pub(super) fn advance_ground_attack(
        &mut self,
        unit: HandleId,
        point: [f32; 2],
        now: f64,
    ) -> Result<(), Error> {
        let Some(record) = self.live(unit) else {
            return Ok(());
        };
        let combat = record.combat.as_ref().expect("live combat");
        let (Some(weapon), Some(position)) = (combat.weapon, record.position) else {
            return Ok(());
        };
        let Some(splash) = weapon.splash else {
            return Err(self.ground_fail("weapon lost its splash columns"));
        };
        let (ready_at, release, chase) = (combat.ready_at, combat.ground_swing, combat.chase);
        let moving = record.activity.as_ref().is_some_and(|a| a.moving());
        let reach = weapon.range + self.unit_collision_radius(unit, NAME)?;
        let gap = distance(position, point);
        if gap > reach {
            if chase != Some(point) || !moving {
                self.steer_unit_in_range(unit, Some(point), reach, NAME)?;
                if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                    record.combat.as_mut().expect("combat").chase = Some(point);
                }
            }
            return Ok(());
        }
        if moving {
            self.steer_unit(unit, None, NAME)?;
        }
        if gap < weapon.min_range {
            return Err(self.ground_fail(format!(
                "target is {gap} from the shooter, inside minRange {}; the original's reaction is not established",
                weapon.min_range
            )));
        }
        match release {
            None if now >= ready_at => {
                let speed = self.hero_combat_bonuses(unit)?.speed - self.curse_attack_penalty(unit);
                if !speed.is_finite() || speed <= 0.0 {
                    return Err(self.ground_fail("attack speed multiplier is not positive"));
                }
                let facing = (point[1] - position[1])
                    .atan2(point[0] - position[0])
                    .to_degrees()
                    .rem_euclid(360.0);
                if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                    record.face = facing;
                    let combat = record.combat.as_mut().expect("combat");
                    combat.ground_swing = Some(now + f64::from(weapon.damage_point / speed));
                    combat.ready_at = now + f64::from(weapon.cooldown / speed);
                    combat.chase = None;
                    if let Some(animation) = &mut record.animation {
                        animation
                            .select_engine(ATTACK_SEQUENCE, now, None)
                            .map_err(|e| self.inner.error(NAME, Failure::UnitAnimation(e)))?;
                    }
                }
            }
            Some(at) if now >= at => {
                let base = self.roll_base_damage(unit, weapon)?;
                if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                    record.combat.as_mut().expect("combat").ground_swing = None;
                }
                match weapon.missile_speed {
                    Some(speed) => {
                        let missiles = &mut self.combat.as_mut().expect("combat").ground_missiles;
                        missiles
                            .try_reserve(1)
                            .map_err(|_| self.inner.error(NAME, Failure::Capacity))?;
                        missiles.push(GroundMissile {
                            source: unit,
                            point,
                            impact_at: now + f64::from(gap / speed),
                            base,
                            weapon,
                        });
                    }
                    None => self.ground_impact(unit, point, base, weapon, splash)?,
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Land every shot due at `now`, in launch order.
    pub(super) fn advance_ground_missiles(&mut self, now: f64) -> Result<(), Error> {
        let pending = std::mem::take(&mut self.combat.as_mut().expect("combat").ground_missiles);
        let (due, later): (Vec<_>, Vec<_>) = pending.into_iter().partition(|m| m.impact_at <= now);
        self.combat.as_mut().expect("combat").ground_missiles = later;
        for missile in due {
            let splash = missile
                .weapon
                .splash
                .ok_or_else(|| self.ground_fail("attack ground weapon has no splash columns"))?;
            self.ground_impact(
                missile.source,
                missile.point,
                missile.base,
                missile.weapon,
                splash,
            )?;
        }
        Ok(())
    }

    fn ground_impact(
        &mut self,
        source: HandleId,
        point: [f32; 2],
        base: f32,
        weapon: Weapon,
        splash: Splash,
    ) -> Result<(), Error> {
        if splash.targets.unsupported {
            return Err(self.ground_fail("splashTargs1 holds a token with no established meaning"));
        }
        let owner = self.executed_unit(source)?.owner;
        let mut hits = Vec::new();
        let victims: Vec<_> = self.executed_units().map(|(id, _)| id).collect();
        for id in victims {
            let Some(victim) = self.weapon_target(id) else {
                continue;
            };
            let Some(position) = victim.position else {
                continue;
            };
            let gap = distance(position, point);
            if gap > splash.quarter {
                continue;
            }
            let structure = super::unit_types::structure(&victim.definition)
                .map_err(|detail| self.ground_fail(detail))?;
            let flying = super::unit_positions::text(
                &victim.definition,
                pcx_objects::editor::SlkId::UnitData,
                "movetp",
            )
            .unwrap_or("")
                == "fly";
            let struck = if structure {
                splash.targets.structure
            } else if flying {
                splash.targets.air
            } else {
                splash.targets.ground
            };
            if !struck {
                continue;
            }
            if (splash.targets.enemy && !self.hostile_owners(owner, victim.owner))
                || (splash.targets.not_self && id == source)
            {
                continue;
            }
            let factor = if gap <= splash.full {
                1.0
            } else if gap <= splash.half {
                splash.half_factor
            } else {
                splash.quarter_factor
            };
            let (armor, defense) = self.victim_armor(id)?;
            hits.push((
                id,
                self.armored_damage(base, weapon, armor, defense) * factor,
            ));
        }
        for (id, damage) in hits {
            self.apply_damage(source, id, damage, true)?;
        }
        Ok(())
    }
}

//! On-hit effects of held orbs (`AIfb` Orb of Fire and Orb of Kil'jaeden, `AIcb` Orb of
//! Corruption). The decoded rules and their evidence are in `docs/re-notes/passive-held-items.md`.
//!
//! Every orb adds its `DataA1` to the holder's attack damage like any `AIat` item (that part is
//! the held flat damage). On a hit that the orb arms, the reviewed client then runs the orb's own
//! effect before it applies the attack damage:
//!
//! - Fire: `DataA1` damage to every living enemy of the attacker within `Area1` of the target,
//!   sparing the target. The damage is universal type, so armor and magic immunity do not apply,
//!   and its attack type is the spells row of the damage-bonus table.
//! - Corruption: armor penalty `DataB1` on the target for `Dur1` (`HeroDur1` for a hero). A
//!   second hit replaces the penalty and restarts the timer; the penalty never stacks.
//!
//! The orb arms when the target's class is listed in the orb row's `targs1` (so a structure is
//! outside the stock orbs). Which unit-state flags the client also tests are open; see the note.
use super::combat::Weapon;
use super::unit_buff::{ActiveBuff, BuffTraits};
use super::{Error, Failure, ObjectHost};
use crate::inventory_powerup::{Orb, OrbKind};
use pcx_script::HandleId;
use pcx_sim::world::Definition;
use std::collections::BTreeMap;

/// Attack-type name of the splash record, whose index in the client's damage-bonus table is zero.
const SPLASH_ATTACK_TYPE: &str = "spells";
const NAME: &str = "orb hit";

struct ActivePenalty {
    buff: [u8; 4],
    points: i32,
    expires_at: f64,
}

#[derive(Default)]
pub(super) struct State {
    penalties: BTreeMap<HandleId, ActivePenalty>,
}

impl ObjectHost {
    fn orb_error(&self, reason: impl Into<String>) -> Error {
        self.inner.error(NAME, Failure::UnitActivity(reason.into()))
    }

    /// Armor removed from the unit by an active orb penalty.
    pub(crate) fn orb_armor_penalty(&self, unit: HandleId) -> f32 {
        self.orbs
            .penalties
            .get(&unit)
            .map_or(0.0, |penalty| penalty.points as f32)
    }

    /// The orb the attacker's hit on `target` arms, if any: the held orb whose `targs1` lists
    /// the target's class.
    pub(super) fn orb_for_hit(
        &self,
        source: HandleId,
        target: HandleId,
    ) -> Result<Option<Orb>, Error> {
        let Some(orb) = self.unit_hero_bonus(source)?.orb else {
            return Ok(None);
        };
        let covered = self.orb_covers(&orb, &self.executed_unit(target)?.definition)?;
        Ok(covered.then_some(orb))
    }

    /// The weapon `unit` attacks `target` with. A held orb enables the unit's second attack
    /// (`DataE1` of 2); against an air target that weapon 1 cannot reach, a second weapon that
    /// names air replaces it, with its own range, cooldown, damage and missile.
    pub(super) fn weapon_against(
        &self,
        unit: HandleId,
        first: Weapon,
        target: HandleId,
    ) -> Result<Weapon, Error> {
        if first.reaches_air != Some(false)
            || !super::unit_types::flying(&self.executed_unit(target)?.definition)
            || self.unit_hero_bonus(unit)?.orb.is_none()
        {
            return Ok(first);
        }
        let second = super::combat::read_weapon(&self.executed_unit(unit)?.definition, 2)
            .map_err(|e| self.orb_error(e))?;
        Ok(second
            .filter(|weapon| weapon.reaches_air == Some(true))
            .unwrap_or(first))
    }

    /// Whether the orb row's `targs1` lists the unit class of `victim`: structure, air (a
    /// `movetp` of `fly`) or ground.
    fn orb_covers(&self, orb: &Orb, victim: &Definition) -> Result<bool, Error> {
        let structure = super::unit_types::structure(victim).map_err(|e| self.orb_error(e))?;
        Ok(if structure {
            orb.targets.structure
        } else if super::unit_types::flying(victim) {
            orb.targets.air
        } else {
            orb.targets.ground
        })
    }

    /// Whether the orb changes the target's armor before the attack damage is computed.
    pub(super) fn orb_corrupts(orb: &Orb) -> bool {
        matches!(orb.kind, OrbKind::Corruption { .. })
    }

    /// Applies or refreshes the corruption penalty on `target`. Other orbs do nothing here.
    pub(super) fn orb_corrupt(&mut self, orb: &Orb, target: HandleId) -> Result<(), Error> {
        let OrbKind::Corruption {
            armor_penalty,
            duration,
            hero_duration,
            buff,
        } = orb.kind
        else {
            return Ok(());
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.orb_error("clock missing"))?;
        let hero = self.executed_unit(target)?.definition.rawcode[0].is_ascii_uppercase();
        let expires_at = now + f64::from(if hero { hero_duration } else { duration });
        let refresh = self.orbs.penalties.contains_key(&target);
        if !refresh {
            self.attach_unit_buff(
                target,
                ActiveBuff::new(
                    buff,
                    BuffTraits {
                        negative: true,
                        ..BuffTraits::default()
                    },
                ),
            )?;
        }
        self.orbs.penalties.insert(
            target,
            ActivePenalty {
                buff,
                points: armor_penalty,
                expires_at,
            },
        );
        Ok(())
    }

    /// Fire orb splash around `target`: the full `DataA1` to each living enemy of `source`
    /// within `Area1` plus its collision radius of the target's position, never the target.
    pub(super) fn orb_splash(
        &mut self,
        orb: &Orb,
        source: HandleId,
        target: HandleId,
    ) -> Result<(), Error> {
        let OrbKind::Fire { area } = orb.kind else {
            return Ok(());
        };
        let Some(center) = self.weapon_target(target).and_then(|t| t.position) else {
            return Ok(());
        };
        let owner = self.executed_unit(source)?.owner;
        let victims: Vec<_> = self.executed_units().map(|(id, _)| id).collect();
        let mut hits = Vec::new();
        for id in victims {
            if id == target {
                continue;
            }
            let Some(victim) = self.weapon_target(id) else {
                continue;
            };
            let (Some(position), Some(combat)) = (victim.position, victim.combat.as_ref()) else {
                continue;
            };
            if !self.hostile_owners(owner, victim.owner) {
                continue;
            }
            if !self.orb_covers(orb, &victim.definition)? {
                continue;
            }
            let defense = combat.defense;
            let reach = area + self.unit_collision_radius(id, NAME)?;
            if super::combat::distance(position, center) > reach {
                continue;
            }
            let damage = self
                .attack_type_damage(SPLASH_ATTACK_TYPE, orb.damage as f32, defense)
                .ok_or_else(|| self.orb_error("the damage-bonus table has no spells row"))?;
            hits.push((id, damage));
        }
        for (id, damage) in hits {
            self.apply_damage(source, id, damage, true)?;
        }
        Ok(())
    }

    /// Ends every penalty whose timer has run out.
    pub(super) fn advance_orbs(&mut self, now: f64) {
        let due: Vec<HandleId> = self
            .orbs
            .penalties
            .iter()
            .filter(|(_, penalty)| penalty.expires_at <= now)
            .map(|(unit, _)| *unit)
            .collect();
        for unit in due {
            self.end_orb_penalty(unit);
        }
    }

    fn end_orb_penalty(&mut self, unit: HandleId) {
        if let Some(penalty) = self.orbs.penalties.remove(&unit) {
            self.detach_unit_buff(unit, penalty.buff);
        }
    }

    /// A buff removal (`UnitRemoveBuffs`) ends the penalty its buff carries.
    pub(super) fn orb_buff_removed(&mut self, unit: HandleId, rawcode: [u8; 4]) {
        if self
            .orbs
            .penalties
            .get(&unit)
            .is_some_and(|penalty| penalty.buff == rawcode)
        {
            self.orbs.penalties.remove(&unit);
        }
    }

    /// A dead or removed unit keeps no penalty.
    pub(super) fn forget_orb_unit(&mut self, unit: HandleId) {
        self.orbs.penalties.remove(&unit);
    }
}

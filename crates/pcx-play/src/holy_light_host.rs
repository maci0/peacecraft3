//! Holy Light: the learned ability whose `AbilityFunc` `Order` names the admitted order.
//!
//! Rank, mana `Cost`, `Cool`, `Rng`, `DataA` (heal amount) and the `targs`
//! filter come from `Units\AbilityData.slk` for that rank. Reach is the attack
//! reach from `combat_host`. Undead damage is `DataA / UNDEAD_DAMAGE_DIVISOR`,
//! the divisor taken from the tooltip "deal half damage to an enemy Undead unit"
//! (`Units\HumanAbilityStrings.txt`), not from a numeric column. Undead,
//! mechanical and ancient come from the `UnitBalance` `type` list. That list is
//! read through `combat_host::column`, because the placement reader accepts only
//! numeric `UnitBalance` fields.
//!
//! Not implemented: walking into reach (out-of-reach casts return false), cast
//! and projectile presentation, spell-effect events, and the Air/Ground
//! movement filter (`targs` must list both, or the cast refuses).
use super::{Entry, Error, Failure, ObjectHost, combat};
use pcx_objects::editor::SlkId;
use pcx_script::HandleId;
use std::collections::BTreeMap;

/// Native that dispatches the order, used as error context.
const NATIVE: &str = "IssueTargetOrder";
/// Tooltip "deal half damage to an enemy Undead unit".
const UNDEAD_DAMAGE_DIVISOR: f32 = 2.0;
/// `targs` tokens this cast evaluates. Any other token refuses.
const TARGET_TOKENS: [&str; 7] = [
    "air",
    "ground",
    "organic",
    "notself",
    "invu",
    "vuln",
    "nonancient",
];

pub(super) struct Rules {
    /// Admitted order name, matched against `AbilityFunc` `Order` profiles.
    name: String,
    order: i32,
    /// Game time each caster may cast again.
    ready_at: BTreeMap<HandleId, f64>,
}

struct Filter {
    organic: bool,
    notself: bool,
    nonancient: bool,
}

fn target_filter(targs: &str) -> Result<Filter, String> {
    let tokens: Vec<&str> = targs.split(',').map(str::trim).collect();
    if let Some(unknown) = tokens.iter().find(|t| !TARGET_TOKENS.contains(t)) {
        return Err(format!("unsupported Holy Light target token {unknown}"));
    }
    let has = |token: &str| tokens.contains(&token);
    if !(has("air") && has("ground") && has("invu") && has("vuln")) {
        return Err("unsupported Holy Light target flags".into());
    }
    Ok(Filter {
        organic: has("organic"),
        notself: has("notself"),
        nonancient: has("nonancient"),
    })
}

enum Effect {
    Heal(f32),
    Damage(f32),
}

impl ObjectHost {
    fn holy_error(&self, reason: String) -> Error {
        self.inner.error(NATIVE, Failure::HeroSkills(reason))
    }

    /// Needs combat (which dispatches the order) and hero skills (which resolve ranks).
    /// `order` is the registered name of the Holy Light order.
    pub fn admit_holy_light(&mut self, order: &str) -> Result<(), Error> {
        if self.holy_light.is_some() || self.combat.is_none() || !self.has_hero_skills() {
            return Err(
                self.holy_error("holy light needs combat and hero skills, admitted once".into())
            );
        }
        let id = self
            .order_registry
            .as_ref()
            .and_then(|r| r.id(order))
            .filter(|id| *id != -1)
            .ok_or_else(|| self.holy_error(format!("order {order} is not registered")))?;
        self.holy_light = Some(Rules {
            name: order.into(),
            order: id,
            ready_at: BTreeMap::new(),
        });
        Ok(())
    }

    /// The order number that casts Holy Light, when admitted.
    pub(super) fn holy_light_order(&self) -> Option<i32> {
        self.holy_light.as_ref().map(|rules| rules.order)
    }

    fn unit_has_class(&self, unit: HandleId, class: &str) -> Result<bool, Error> {
        let definition = &self.executed_unit(unit)?.definition;
        let Some(value) = combat::column(definition, SlkId::UnitBalance, "type")
            .map_err(|e| self.inner.error(NATIVE, Failure::UnitActivity(e)))?
        else {
            return Ok(false);
        };
        let list = combat::text(value, "type")
            .map_err(|e| self.inner.error(NATIVE, Failure::UnitActivity(e)))?;
        Ok(list
            .split(',')
            .any(|t| t.trim().eq_ignore_ascii_case(class)))
    }

    /// Casts on a live target in reach. Returns false when a rule refuses (not
    /// learned, cooldown, mana, reach, target filter), true when the effect applied.
    pub(super) fn cast_holy_light(
        &mut self,
        caster: HandleId,
        target: HandleId,
    ) -> Result<bool, Error> {
        let now = self.timer_game_time().ok_or_else(|| {
            self.inner
                .error(NATIVE, Failure::UnitState("clock missing"))
        })?;
        let Some(rules) = self.holy_light.as_ref() else {
            return Ok(false);
        };
        if rules
            .ready_at
            .get(&caster)
            .is_some_and(|ready| now < *ready)
        {
            return Ok(false);
        }
        let order_name = rules.name.clone();
        let Some(caster_unit) = self.live(caster) else {
            return Ok(false);
        };
        let Some(abilities) = caster_unit.abilities.as_ref() else {
            return Ok(false);
        };
        let Some((alias, rank)) = self
            .learned_ability_for_order(abilities, &order_name)
            .map_err(|e| self.holy_error(e))?
        else {
            return Ok(false);
        };
        let levels = self
            .hero_ability_number(alias, "levels")
            .map_err(|e| self.holy_error(e))?;
        if !(1..=levels).contains(&rank) {
            return Err(self.holy_error(format!("rank {rank} outside {levels} levels")));
        }
        let column = |stem: &str| {
            self.hero_ability_number(alias, &format!("{stem}{rank}"))
                .map_err(|e| self.holy_error(e))
        };
        let cost = column("Cost")? as f32;
        let cooldown = f64::from(column("Cool")?);
        let range = column("Rng")? as f32;
        let amount = column("DataA")? as f32;
        let targs = self
            .hero_ability_text(alias, &format!("targs{rank}"))
            .map_err(|e| self.holy_error(e))?;
        let filter = target_filter(&targs).map_err(|e| self.holy_error(e))?;

        let mana = caster_unit
            .vitals
            .ok_or_else(|| {
                self.inner
                    .error(NATIVE, Failure::UnitState("caster vitals missing"))
            })?
            .mana;
        if mana < cost {
            return Ok(false);
        }
        let Some(target_unit) = self.live(target) else {
            return Ok(false);
        };
        if filter.notself && caster == target {
            return Ok(false);
        }
        let (Some(from), Some(to)) = (caster_unit.position, target_unit.position) else {
            return Err(self
                .inner
                .error(NATIVE, Failure::UnitState("unit position missing")));
        };
        let reach = range
            + self.unit_collision_radius(caster, NATIVE)?
            + self.unit_collision_radius(target, NATIVE)?;
        if combat::distance(from, to) > reach {
            return Ok(false);
        }
        let undead = self.unit_has_class(target, "undead")?;
        let mechanical = self.unit_has_class(target, "mechanical")?;
        if filter.nonancient && self.unit_has_class(target, "ancient")? {
            return Ok(false);
        }
        let effect = if self.is_enemy(caster, target) {
            if !undead {
                return Ok(false);
            }
            Effect::Damage(amount / UNDEAD_DAMAGE_DIVISOR)
        } else {
            if filter.organic && (undead || mechanical) {
                return Ok(false);
            }
            Effect::Heal(amount)
        };

        self.clear_combat_target(caster);
        if let Entry::Unit { record } = self.inner.entry_mut(caster)?
            && let Some(vitals) = &mut record.vitals
        {
            vitals.mana -= cost;
        }
        if let Some(rules) = &mut self.holy_light {
            rules.ready_at.insert(caster, now + cooldown);
        }
        match effect {
            Effect::Damage(damage) => self.deal_weapon_damage(caster, target, damage)?,
            Effect::Heal(amount) => {
                if let Entry::Unit { record } = self.inner.entry_mut(target)?
                    && let Some(vitals) = &mut record.vitals
                {
                    vitals.life = (vitals.life + amount).min(vitals.max_life);
                }
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_filter_requires_the_authored_flags_and_refuses_unknown_tokens() {
        let all = "air,ground,organic,notself,invu,vuln,nonancient";
        let filter = target_filter(all).unwrap();
        assert!(filter.organic && filter.notself && filter.nonancient);
        let no_organic = target_filter("air,ground,invu,vuln").unwrap();
        assert!(!no_organic.organic && !no_organic.notself && !no_organic.nonancient);
        assert!(target_filter("air,ground,invu,vuln,bogus").is_err());
        assert!(target_filter("air,ground,organic").is_err());
    }
}

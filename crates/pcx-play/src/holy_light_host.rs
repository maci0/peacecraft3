//! Holy Light (`AHhb`): a timed heal of a friendly living unit or damage to an enemy undead unit.
//!
//! The rank's `levels`, `Cost`, `Cool`, `Rng`, `DataA` and `targs` come from
//! `Units\AbilityData.slk`. The order string comes from the row's `AbilityFunc`
//! `Order`. A cast that is out of range walks the caster into range first.
//! Undead damage is `DataA / UNDEAD_DAMAGE_DIVISOR`; the divisor is the tooltip
//! wording "half damage" (`Units\HumanAbilityStrings.txt`), not a numeric column.
//!
//! Not implemented: cast and target art (`Targetart` is not attached), and the
//! Air/Ground movement filter (`targs` must list both tokens, or the cast refuses).
//! Cast range is measured between unit positions, without collision radii; that
//! geometry is not yet qualified against the native cast-range check.
use super::{Entry, Error, EventSubject, Failure, ObjectHost, combat};
use pcx_objects::editor::SlkId;
use pcx_script::HandleId;
use std::collections::BTreeMap;

/// The implementation is bound to this row, as the campaign selects it with
/// `SelectHeroSkill(unit, 'AHhb')`. Its order string is read from the row.
const ABILITY: [u8; 4] = *b"AHhb";
/// Native that dispatches the order, used as error context.
const NATIVE: &str = "IssueTargetOrder";
/// Animation sequence played on cast when the unit model has one.
const CAST_SEQUENCE: &str = "spell";
/// Tooltip "deal half damage to an enemy Undead unit" (`Units\HumanAbilityStrings.txt`).
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
    order: i32,
    /// Game time each caster may cast again.
    ready_at: BTreeMap<HandleId, f64>,
    /// Accepted casts still walking into range, by caster.
    pending: BTreeMap<HandleId, HandleId>,
}

struct Filter {
    organic: bool,
    notself: bool,
    nonancient: bool,
}

struct Spell {
    cost: f32,
    cooldown: f64,
    range: f32,
    amount: f32,
    filter: Filter,
}

/// Holy Light as a player command: the hotkey that arms it, and the order number a target click issues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    pub hotkey: char,
    pub order: i32,
}

#[derive(Clone, Copy)]
enum Effect {
    Heal(f32),
    Damage(f32),
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

impl ObjectHost {
    fn holy_error(&self, reason: String) -> Error {
        self.inner.error(NATIVE, Failure::HeroSkills(reason))
    }

    /// Needs combat (which dispatches the order) and hero skills (which resolve ranks).
    pub fn admit_holy_light(&mut self) -> Result<(), Error> {
        if self.holy_light.is_some() || self.combat.is_none() || !self.has_hero_skills() {
            return Err(
                self.holy_error("holy light needs combat and hero skills, admitted once".into())
            );
        }
        let name = self
            .hero_ability_profile(ABILITY, "Order")
            .map_err(|e| self.holy_error(e))?;
        let order = self
            .order_registry
            .as_ref()
            .and_then(|r| r.id(&name))
            .filter(|id| *id != -1)
            .ok_or_else(|| self.holy_error(format!("order {name} is not registered")))?;
        self.holy_light = Some(Rules {
            order,
            ready_at: BTreeMap::new(),
            pending: BTreeMap::new(),
        });
        Ok(())
    }

    /// The command this unit gets from a learned Holy Light, or `None` without a learned rank.
    /// The hotkey is the row's `Hotkey` profile, a single letter.
    pub fn holy_light_command(&self, unit: HandleId) -> Result<Option<Command>, Error> {
        let Some(rules) = self.holy_light.as_ref() else {
            return Ok(None);
        };
        let record = self.executed_unit(unit)?;
        if record
            .abilities
            .as_ref()
            .is_none_or(|a| a.level(ABILITY) <= 0)
        {
            return Ok(None);
        }
        let hotkey = self
            .hero_ability_profile(ABILITY, "Hotkey")
            .map_err(|e| self.holy_error(e))?;
        match hotkey.as_bytes() {
            [letter] if letter.is_ascii_alphabetic() => Ok(Some(Command {
                hotkey: char::from(letter.to_ascii_uppercase()),
                order: rules.order,
            })),
            _ => Err(self.holy_error(format!("Hotkey {hotkey:?} is not one letter"))),
        }
    }

    /// The order number that casts Holy Light, when admitted.
    pub(super) fn holy_light_order(&self) -> Option<i32> {
        self.holy_light.as_ref().map(|rules| rules.order)
    }

    /// Drops a walking cast when its caster takes another order.
    pub(super) fn forget_holy_cast(&mut self, caster: HandleId) {
        if let Some(rules) = &mut self.holy_light {
            rules.pending.remove(&caster);
        }
    }

    /// The learned rank's values, or `None` when the caster is not a live unit with the ability.
    fn holy_spell(&self, caster: HandleId) -> Result<Option<Spell>, Error> {
        let Some(unit) = self.live(caster) else {
            return Ok(None);
        };
        let Some(abilities) = unit.abilities.as_ref() else {
            return Ok(None);
        };
        let rank = abilities.level(ABILITY);
        if rank <= 0 {
            return Ok(None);
        }
        let levels = self
            .hero_ability_number(ABILITY, "levels")
            .map_err(|e| self.holy_error(e))?;
        if rank > levels {
            return Err(self.holy_error(format!("rank {rank} outside {levels} levels")));
        }
        let number = |stem: &str| {
            self.hero_ability_number(ABILITY, &format!("{stem}{rank}"))
                .map_err(|e| self.holy_error(e))
        };
        let targs = self
            .hero_ability_text(ABILITY, &format!("targs{rank}"))
            .map_err(|e| self.holy_error(e))?;
        Ok(Some(Spell {
            cost: number("Cost")? as f32,
            cooldown: f64::from(number("Cool")?),
            range: number("Rng")? as f32,
            amount: number("DataA")? as f32,
            filter: target_filter(&targs).map_err(|e| self.holy_error(e))?,
        }))
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

    /// The effect this spell would have on `target`, or `None` when the target is not
    /// eligible. Checks liveness, self, ancient and the enemy/friend class rules.
    fn holy_effect(
        &self,
        caster: HandleId,
        target: HandleId,
        spell: &Spell,
    ) -> Result<Option<Effect>, Error> {
        if self.live(target).is_none() || (spell.filter.notself && caster == target) {
            return Ok(None);
        }
        let undead = self.unit_has_class(target, "undead")?;
        if spell.filter.nonancient && self.unit_has_class(target, "ancient")? {
            return Ok(None);
        }
        if self.is_enemy(caster, target) {
            return Ok((undead).then_some(Effect::Damage(spell.amount / UNDEAD_DAMAGE_DIVISOR)));
        }
        let mechanical = self.unit_has_class(target, "mechanical")?;
        let neutral = self.neutral_passive_owner(target);
        let organic = !undead && !mechanical;
        Ok((!neutral && (organic || !spell.filter.organic)).then_some(Effect::Heal(spell.amount)))
    }

    fn holy_cooling_or_short_of_mana(&self, caster: HandleId, spell: &Spell, now: f64) -> bool {
        let Some(rules) = self.holy_light.as_ref() else {
            return true;
        };
        let cooling = rules
            .ready_at
            .get(&caster)
            .is_some_and(|ready| now < *ready);
        let short = self
            .live(caster)
            .and_then(|unit| unit.vitals)
            .is_none_or(|vitals| vitals.mana < spell.cost);
        cooling || short
    }

    /// `IssueTargetOrder` for Holy Light. The order is accepted when the caster can
    /// learn, afford and target it now; it then walks into range and casts there.
    pub(super) fn order_holy_light(
        &mut self,
        caster: HandleId,
        target: HandleId,
    ) -> Result<bool, Error> {
        let now = self.timer_game_time().ok_or_else(|| {
            self.inner
                .error(NATIVE, Failure::UnitState("clock missing"))
        })?;
        let Some(spell) = self.holy_spell(caster)? else {
            return Ok(false);
        };
        if self.holy_effect(caster, target, &spell)?.is_none()
            || self.holy_cooling_or_short_of_mana(caster, &spell, now)
        {
            return Ok(false);
        }
        self.clear_combat_target(caster);
        if let Some(rules) = &mut self.holy_light {
            rules.pending.insert(caster, target);
        }
        self.step_holy_cast(caster, target, now)?;
        Ok(true)
    }

    /// Every accepted cast advances once per game step.
    pub(super) fn advance_holy_light(&mut self, now: f64) -> Result<(), Error> {
        let Some(rules) = self.holy_light.as_ref() else {
            return Ok(());
        };
        let pending: Vec<(HandleId, HandleId)> = rules
            .pending
            .iter()
            .map(|(&caster, &target)| (caster, target))
            .collect();
        for (caster, target) in pending {
            self.step_holy_cast(caster, target, now)?;
        }
        Ok(())
    }

    fn step_holy_cast(
        &mut self,
        caster: HandleId,
        target: HandleId,
        now: f64,
    ) -> Result<(), Error> {
        let spell = self.holy_spell(caster)?;
        let effect = match &spell {
            Some(spell) => self.holy_effect(caster, target, spell)?,
            None => None,
        };
        let (Some(spell), Some(effect)) = (spell, effect) else {
            return self.cancel_holy_cast(caster);
        };
        if self.holy_cooling_or_short_of_mana(caster, &spell, now) {
            return self.cancel_holy_cast(caster);
        }
        let (Some(from), Some(to)) = (
            self.live(caster).and_then(|u| u.position),
            self.live(target).and_then(|u| u.position),
        ) else {
            return Err(self
                .inner
                .error(NATIVE, Failure::UnitState("unit position missing")));
        };
        if combat::distance(from, to) <= spell.range {
            if self
                .live(caster)
                .is_some_and(|u| u.activity.as_ref().is_some_and(|a| a.moving()))
            {
                self.steer_unit(caster, None, NATIVE)?;
            }
            return self.apply_holy_cast(caster, target, &spell, effect, now);
        }
        let idle = !self
            .live(caster)
            .and_then(|u| u.activity.as_ref())
            .is_some_and(|a| a.moving());
        if idle {
            self.steer_unit(caster, Some(to), NATIVE)?;
        }
        Ok(())
    }

    /// The walking cast ends: its caster stops moving toward the target.
    fn cancel_holy_cast(&mut self, caster: HandleId) -> Result<(), Error> {
        self.forget_holy_cast(caster);
        if self.live(caster).is_some() {
            self.steer_unit(caster, None, NATIVE)?;
        }
        Ok(())
    }

    fn apply_holy_cast(
        &mut self,
        caster: HandleId,
        target: HandleId,
        spell: &Spell,
        effect: Effect,
        now: f64,
    ) -> Result<(), Error> {
        self.forget_holy_cast(caster);
        if let Some(rules) = &mut self.holy_light {
            rules.ready_at.insert(caster, now + spell.cooldown);
        }
        self.clear_combat_target(caster);
        if let Entry::Unit { record } = self.inner.entry_mut(caster)?
            && let Some(vitals) = &mut record.vitals
        {
            vitals.mana -= spell.cost;
        }
        let played = match self.inner.entry_mut(caster)? {
            Entry::Unit { record } => match &mut record.animation {
                Some(animation) if animation.has_sequence(CAST_SEQUENCE) => {
                    animation.select(CAST_SEQUENCE, now)
                }
                _ => Ok(()),
            },
            _ => Ok(()),
        };
        played.map_err(|e| self.inner.error(NATIVE, Failure::UnitAnimation(e)))?;
        match effect {
            Effect::Damage(damage) => self.damage_unit(caster, target, damage)?,
            Effect::Heal(amount) => {
                if let Entry::Unit { record } = self.inner.entry_mut(target)?
                    && let Some(vitals) = &mut record.vitals
                {
                    vitals.life = (vitals.life + amount).min(vitals.max_life);
                }
            }
        }
        self.queue_spell_effect(caster, target)
    }

    /// `EVENT_UNIT_SPELL_EFFECT` and `EVENT_PLAYER_UNIT_SPELL_EFFECT` for the caster,
    /// each naming the target through `GetSpellTargetUnit`.
    fn queue_spell_effect(&mut self, caster: HandleId, target: HandleId) -> Result<(), Error> {
        let unit_event = self.source_constant_enumerand("EVENT_UNIT_SPELL_EFFECT", "unitevent")?;
        let player_event =
            self.source_constant_enumerand("EVENT_PLAYER_UNIT_SPELL_EFFECT", "playerunitevent")?;
        let owner = self.executed_unit(caster)?.owner;
        let subject = EventSubject {
            unit: Some(caster),
            spell_target: Some(target),
            ..EventSubject::default()
        };
        let mut fired = Vec::new();
        for entry in &self.inner.arena {
            match entry {
                Entry::RegisteredUnitEvent {
                    trigger,
                    unit,
                    unitevent,
                    enumerand,
                } if *unit == caster && *enumerand == unit_event => {
                    fired.push((*trigger, None, Some(*unitevent), None));
                }
                Entry::RegisteredPlayerUnitEvent(r)
                    if r.player == owner && r.enumerand == player_event =>
                {
                    fired.push((r.trigger, r.filter, Some(r.playerunitevent), Some(r.player)));
                }
                _ => {}
            }
        }
        for (trigger, filter, event, player) in fired {
            self.queue_event(trigger, filter, EventSubject { player, ..subject }, event)?;
        }
        Ok(())
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

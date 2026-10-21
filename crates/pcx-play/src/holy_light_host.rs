//! Holy Light (`AHhb`): a timed heal of an allied living unit, or damage to an enemy undead unit.
//!
//! Per learned rank, `levels`, `Cost`, `Cool`, `Rng`, `DataA` and `targs` come from
//! `Units\AbilityData.slk`. The order string is the row's `AbilityFunc` `Order`, the
//! hotkey its `Hotkey`, and the art its `Targetart` with `Targetattach`. Undead damage
//! is `DataA / 2`, admitted only when each `UberTip` names "half damage".
//!
//! Ally, enemy and undead classes use the explicit player alliance table and the
//! `UnitBalance` `type` list. Cast range is measured between unit positions with no
//! collision radii. Qualifying that geometry needs native range evidence.
//!
//! Not implemented: the spell's armor and attack type. Undead damage bypasses target
//! armor until a native check qualifies it. Art lifetime uses the admitted model's
//! non-looping sequence; target removal also retires it. Air/Ground filters
//! require both `targs` tokens.
use super::{Entry, Error, Failure, ObjectHost, combat};
use pcx_objects::editor::SlkId;
use pcx_script::HandleId;
use std::collections::BTreeMap;

/// The implementation is bound to this row, as the campaign selects it with
/// `SelectHeroSkill(unit, 'AHhb')`. Its order and art come from the row.
const ABILITY: [u8; 4] = crate::map_startup_host::HOLY_LIGHT_ROW;
/// Native that dispatches the order, used as error context.
const NATIVE: &str = "IssueTargetOrder";
/// Animation sequence played on cast when the unit model has one.
const CAST_SEQUENCE: &str = "spell";
/// Attach point used when the row authors no `Targetattach`. Inference: the model root.
const DEFAULT_ATTACH: &str = "origin";
/// `UberTip` wording that names the undead ratio, and the divisor it stands for.
const UNDEAD_RATIO_TEXT: &str = "half damage";
const HALF: f32 = 2.0;
const MILLISECONDS_PER_SECOND: f64 = 1000.0;
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
    undead_divisor: f32,
    art: Art,
    /// Game time each caster may cast again.
    ready_at: BTreeMap<HandleId, f64>,
    /// Accepted casts still walking into range, by caster.
    pending: BTreeMap<HandleId, HandleId>,
}

#[derive(Clone)]
struct Art {
    model: String,
    attach: String,
    /// Duration of the model's only sequence, which is non-looping, in milliseconds.
    lifetime_ms: f64,
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
    undead_divisor: f32,
    filter: Filter,
}

#[derive(Clone, Copy)]
enum Effect {
    Heal(f32),
    Damage(f32),
}

/// A player command: the hotkey that arms it, and the order number a target click issues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    /// Implemented ability row supplying its order and presentation.
    pub ability: [u8; 4],
    pub hotkey: char,
    pub order: i32,
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

/// The single sequence of the art model must be non-looping; its interval is the lifetime.
/// Any other sequence layout refuses, because which sequence the engine plays is not established.
fn art_lifetime(model: &[u8]) -> Result<f64, String> {
    let parsed = pcx_mdl::Mdlx::parse(model).map_err(|e| format!("art model: {e}"))?;
    let sequences = parsed
        .sequences(model)
        .map_err(|e| format!("art model SEQS: {e}"))?;
    match sequences.as_slice() {
        [only] if only.non_looping && only.duration() > 0 => Ok(f64::from(only.duration())),
        _ => Err("art model needs exactly one non-looping sequence".into()),
    }
}

impl ObjectHost {
    fn holy_error(&self, reason: String) -> Error {
        self.inner.error(NATIVE, Failure::HeroSkills(reason))
    }

    /// Needs combat (dispatches the order), hero skills (resolves ranks) and unit
    /// effects (shows the `Targetart`). Every profile it reads must be authored.
    pub fn admit_holy_light(&mut self, art_model: &[u8]) -> Result<(), Error> {
        if self.holy_light.is_some()
            || self.combat.is_none()
            || !self.has_hero_skills()
            || !self.effects
        {
            return Err(self.holy_error(
                "holy light needs combat, hero skills and unit effects, admitted once".into(),
            ));
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
        let model = self
            .hero_ability_profile(ABILITY, "Targetart")
            .map_err(|e| self.holy_error(e))?;
        let attach = self
            .hero_ability_profiles(ABILITY, "Targetattach")
            .map_err(|e| self.holy_error(e))?
            .into_iter()
            .next()
            .unwrap_or_else(|| DEFAULT_ATTACH.into());
        let tips = self
            .hero_ability_profiles(ABILITY, "UberTip")
            .map_err(|e| self.holy_error(e))?;
        if tips.is_empty() || !tips.iter().all(|tip| tip.contains(UNDEAD_RATIO_TEXT)) {
            return Err(self.holy_error(format!("UberTip does not state the {UNDEAD_RATIO_TEXT}")));
        }
        let lifetime_ms = art_lifetime(art_model).map_err(|e| self.holy_error(e))?;
        self.holy_light = Some(Rules {
            order,
            undead_divisor: HALF,
            art: Art {
                model,
                attach,
                lifetime_ms,
            },
            ready_at: BTreeMap::new(),
            pending: BTreeMap::new(),
        });
        Ok(())
    }

    /// The order number that casts Holy Light, when admitted.
    pub(super) fn holy_light_order(&self) -> Option<i32> {
        self.holy_light.as_ref().map(|rules| rules.order)
    }

    /// The command this unit gets from a learned Holy Light, or `None` without a learned rank.
    pub fn holy_light_command(&self, unit: HandleId) -> Result<Option<Command>, Error> {
        let Some(rules) = self.holy_light.as_ref() else {
            return Ok(None);
        };
        if !self.unit_ability_available(unit, ABILITY)? {
            return Ok(None);
        }
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
                ability: ABILITY,
                hotkey: char::from(letter.to_ascii_uppercase()),
                order: rules.order,
            })),
            _ => Err(self.holy_error(format!("Hotkey {hotkey:?} is not one letter"))),
        }
    }

    /// Read the learned spell on the same game clock and pools used by its cast path.
    pub fn holy_light_readiness(
        &self,
        unit: HandleId,
    ) -> Result<Option<crate::ability_profile::Readiness>, Error> {
        let Some(rules) = self.holy_light.as_ref() else {
            return Ok(None);
        };
        let Some(spell) = self.holy_spell(unit)? else {
            return Ok(None);
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.holy_error("clock missing".into()))?;
        Ok(Some(Self::ability_readiness_at(
            self.executed_unit(unit)?,
            spell.cost,
            rules.ready_at.get(&unit).copied(),
            now,
        )))
    }

    /// Drops a walking cast when its caster dies, is removed or takes another order.
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
        if !self.unit_ability_available(caster, ABILITY)? {
            return Ok(None);
        }
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
            undead_divisor: self.holy_light.as_ref().map_or(HALF, |r| r.undead_divisor),
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
    /// eligible. Enemies must be undead. Allies (same player or passive alliance) must
    /// be living, unless `targs` allows any class.
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
            return Ok(undead.then_some(Effect::Damage(spell.amount / spell.undead_divisor)));
        }
        if !self.is_ally(caster, target) {
            return Ok(None);
        }
        let mechanical = self.unit_has_class(target, "mechanical")?;
        if spell.filter.organic && (undead || mechanical) {
            return Ok(None);
        }
        Ok(Some(Effect::Heal(spell.amount)))
    }

    fn holy_cooling_or_short_of_mana(&self, caster: HandleId, spell: &Spell, now: f64) -> bool {
        let Some(rules) = self.holy_light.as_ref() else {
            return true;
        };
        let Some(unit) = self.live(caster) else {
            return true;
        };
        !Self::ability_readiness_at(unit, spell.cost, rules.ready_at.get(&caster).copied(), now)
            .ready()
    }

    /// `IssueTargetOrder` for Holy Light. The order is accepted when the caster can
    /// learn, afford and target it now; it then walks into range and casts there.
    pub(super) fn order_holy_light(
        &mut self,
        caster: HandleId,
        target: HandleId,
    ) -> Result<bool, Error> {
        if self.executed_unit(caster)?.paused_since.is_some() {
            return Ok(false);
        }
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
        self.wake_natural_sleep(caster);
        self.cancel_patrol(caster);
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
            if self.executed_unit(caster)?.paused_since.is_some() {
                continue;
            }
            self.step_holy_cast(caster, target, now)?;
        }
        Ok(())
    }

    /// Rechecks learned rank, target, cooldown and mana, then casts in range or walks.
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
            if self.is_moving(caster) {
                self.steer_unit(caster, None, NATIVE)?;
            }
            return self.apply_holy_cast(caster, target, &spell, effect, now);
        }
        if !self.is_moving(caster) {
            self.steer_unit(caster, Some(to), NATIVE)?;
        }
        Ok(())
    }

    fn is_moving(&self, unit: HandleId) -> bool {
        self.live(unit)
            .and_then(|u| u.activity.as_ref())
            .is_some_and(|a| a.moving())
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
                    animation.select(CAST_SEQUENCE, now, None)
                }
                _ => Ok(()),
            },
            _ => Ok(()),
        };
        played.map_err(|e| self.inner.error(NATIVE, Failure::UnitAnimation(e)))?;
        match effect {
            Effect::Damage(damage) => self.damage_unit(caster, target, damage)?,
            Effect::Heal(amount) => self.heal_unit(target, amount)?,
        }
        self.attach_holy_art(target, now)?;
        self.queue_spell_effect(caster, Some(target))
    }

    /// Shows the `Targetart` on the target. The previous art on that target ends first.
    /// Each cast attaches its own art with its own expiry. Earlier art on the target keeps its
    /// own lifetime; the effect contract has no per-target replacement.
    fn attach_holy_art(&mut self, target: HandleId, now: f64) -> Result<(), Error> {
        let Some(rules) = self.holy_light.as_ref() else {
            return Ok(());
        };
        let art = rules.art.clone();
        let expires_at = now + art.lifetime_ms / MILLISECONDS_PER_SECOND;
        self.add_unit_effect(&art.model, target, &art.attach, Some(expires_at))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art(sequences: &[(u32, bool)]) -> Vec<u8> {
        let mut bytes = b"MDLX".to_vec();
        bytes.extend(b"VERS");
        bytes.extend(4_u32.to_le_bytes());
        bytes.extend(800_u32.to_le_bytes());
        let mut records = Vec::new();
        for (end, looping) in sequences {
            let mut sequence = vec![0; pcx_mdl::SEQUENCE_SIZE];
            sequence[84..88].copy_from_slice(&end.to_le_bytes());
            sequence[92..96].copy_from_slice(&u32::from(!looping).to_le_bytes());
            records.extend(sequence);
        }
        bytes.extend(b"SEQS");
        bytes.extend((records.len() as u32).to_le_bytes());
        bytes.extend(records);
        bytes
    }

    #[test]
    fn art_lifetime_is_the_only_sequence_when_it_does_not_loop() {
        assert_eq!(art_lifetime(&art(&[(2800, false)])), Ok(2800.0));
        assert!(art_lifetime(&art(&[(2800, true)])).is_err());
        assert!(art_lifetime(&art(&[(2800, false), (900, false)])).is_err());
        assert!(art_lifetime(&art(&[])).is_err());
    }

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

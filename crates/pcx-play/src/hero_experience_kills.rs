use super::*;
use pcx_script::{RealOps, ast::BinOp};

impl ObjectHost {
    /// Preflight the bounded award list before the death mutates world state.
    pub(crate) fn plan_hero_kill_experience(
        &self,
        victim: HandleId,
        killer: Option<HandleId>,
    ) -> Result<Vec<(HandleId, i32)>, Error> {
        let empty = Vec::new();
        let Some(killer) = killer else {
            return Ok(empty);
        };
        let Some(admission) = self.admitted_units()?.heroes.as_ref() else {
            return Ok(empty);
        };
        let Some(rules) = &admission.rules.experience else {
            return Ok(empty);
        };
        let Some((awards, bias)) = &rules.awards else {
            return Ok(empty);
        };
        let fail = |detail: String| self.hero_error("hero kill XP", detail);
        let victim_record = self.created_unit(victim, "hero kill XP")?;
        if victim_record.illusion.is_some() {
            return Ok(empty);
        }
        let killer_owner = self.created_unit(killer, "hero kill XP")?.owner;
        if !self.hostile_owners(killer_owner, victim_record.owner) {
            return Ok(empty);
        }
        if !awards.buildings
            && crate::playercolor_host::objects::unit_types::structure(&victim_record.definition)
                .map_err(fail)?
        {
            return Ok(empty);
        }
        let victim_hero = victim_record
            .abilities
            .as_ref()
            .and_then(|a| a.hero.as_ref());
        let level = match victim_hero {
            Some(hero) => hero.level,
            None => {
                let field = crate::playercolor_host::objects::combat::column(
                    &victim_record.definition,
                    pcx_objects::editor::SlkId::UnitBalance,
                    "level",
                )
                .map_err(fail)?
                .ok_or_else(|| fail("victim UnitBalance/level absent".into()))?;
                // Authored no-level sentinel: the selected client reports level zero, with no XP.
                if matches!(field, FieldValue::Cell(text) if text.trim() == "-") {
                    return Ok(empty);
                }
                let level = crate::playercolor_host::objects::combat::number(field, "level")
                    .map_err(fail)?
                    .ok_or_else(|| fail("victim UnitBalance/level empty".into()))?;
                if level < 0.0 || level.fract() != 0.0 || level >= i32::MAX as f32 {
                    return Err(fail("invalid victim level".into()));
                }
                level as i32
            }
        };
        if level == 0 {
            return Ok(empty);
        }
        let grant = awards.grant(victim_hero.is_some(), level).map_err(fail)?;
        if grant == 0 {
            return Ok(empty);
        }
        let victim_position = victim_record
            .position
            .unwrap_or([victim_record.x, victim_record.y]);
        let summoned = victim_record.summoned;
        let creep = self.creep_owner(victim_record.owner, "hero kill XP")?;
        let source_player = self.player_settings(killer_owner)?;
        let shared = self.source_constant_enumerand("ALLIANCE_SHARED_XP", "alliancetype")?;
        let maximum = admission.rules.progression.max_level;
        let mut nearby = Vec::new();
        let mut global = Vec::new();
        for (unit, record) in self.executed_units() {
            if unit == victim || !record.shown || record.combat.as_ref().is_none_or(|c| c.dead) {
                continue;
            }
            let Some(hero) = record.abilities.as_ref().and_then(|a| a.hero.as_ref()) else {
                continue;
            };
            if hero.level >= maximum && !awards.maximum_drains {
                continue;
            }
            let player = self.player_settings(record.owner)?;
            let own = record.owner == killer_owner;
            if !own && !self.effective_alliance(source_player, player, shared) {
                continue;
            }
            let position = record.position.unwrap_or([record.x, record.y]);
            let dx = f64::from(position[0]) - f64::from(victim_position[0]);
            let dy = f64::from(position[1]) - f64::from(victim_position[1]);
            if dx * dx + dy * dy <= f64::from(awards.range).powi(2) {
                nearby
                    .try_reserve(1)
                    .map_err(|_| fail("nearby hero allocation".into()))?;
                nearby.push(unit);
            }
            if own && awards.global {
                global
                    .try_reserve(1)
                    .map_err(|_| fail("global hero allocation".into()))?;
                global.push(unit);
            }
        }
        let participants = if nearby.is_empty() { global } else { nearby };
        if participants.is_empty() {
            return Ok(empty);
        }
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| fail("numeric policy absent".into()))?;
        let arithmetic = |op, a, b| -> Result<f32, Error> {
            match numeric.binary(op, a, b).map_err(fail)? {
                Value::Real(v) if v.is_finite() && v >= 0.0 => Ok(v),
                _ => Err(fail("invalid award arithmetic".into())),
            }
        };
        let count =
            i32::try_from(participants.len()).map_err(|_| fail("hero count overflow".into()))?;
        let mut share = numeric.integer(grant).map_err(fail)?;
        if summoned {
            share = arithmetic(BinOp::Mul, share, awards.summoned)?;
        }
        share = arithmetic(BinOp::Div, share, numeric.integer(count).map_err(fail)?)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(participants.len())
            .map_err(|_| fail("XP award allocation".into()))?;
        for unit in participants {
            let record = self.created_unit(unit, "hero kill XP")?;
            let hero = record
                .abilities
                .as_ref()
                .and_then(|a| a.hero.as_ref())
                .expect("selected hero");
            if hero.level >= maximum {
                continue;
            }
            let state = hero
                .experience
                .ok_or_else(|| fail("hero XP absent from legacy cache".into()))?;
            if state.suspended {
                continue;
            }
            let mut amount = share;
            if creep {
                amount = arithmetic(
                    BinOp::Mul,
                    amount,
                    awards.creep_factor(hero.level).map_err(fail)?,
                )?;
            }
            if amount == 0.0 {
                continue;
            }
            let handicap = self
                .player_settings(record.owner)?
                .experience_handicap
                .ok_or_else(|| fail("initial player XP handicap is unqualified".into()))?;
            amount = arithmetic(BinOp::Mul, amount, handicap)?;
            let rounded = arithmetic(BinOp::Add, amount, *bias)?.floor();
            if rounded >= i32::MAX as f32 {
                return Err(fail("XP award overflow".into()));
            }
            let xp = state
                .xp
                .checked_add(rounded as i32)
                .ok_or_else(|| fail("hero XP overflow".into()))?;
            if xp != state.xp {
                result.push((unit, xp));
            }
        }
        Ok(result)
    }
}

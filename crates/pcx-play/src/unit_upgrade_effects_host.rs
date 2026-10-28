//! Upgrade effects on a player's units: applied to the owner's existing units when a researched
//! level changes, and to a unit created afterwards. Slots come from the selected `UpgradeData`
//! rows; `crate::upgrade_effects` holds the arithmetic and the list of unestablished effects.
//!
//! Matching unit: owned by the researching player, with the upgrade in its type's `UnitBalance`
//! `upgrades` list (`0x694230`). Pools are written only after every unit projected.
use super::current_research::definition_upgrade_codes;
use super::{Entry, Error, Failure, ObjectHost};
use crate::playercolor_host::{ExecutedUnit, Vitals};
use crate::upgrade_effects::{Effect, Slot, Unapplied, change_max_life, change_max_mana};
use pcx_script::{HandleId, Value};

const NATIVE: &str = "SetPlayerTechResearched";
const MAX_LEVEL_PROJECTIONS: u64 = 1_000_000;

fn projection_fits(from: i32, to: i32, units: usize, effects: usize) -> bool {
    u64::from(from.abs_diff(to))
        .checked_mul(units as u64)
        .and_then(|n| n.checked_mul(effects as u64))
        .is_some_and(|n| n <= MAX_LEVEL_PROJECTIONS)
}

/// Effects of one level change, computed before any write.
struct Plan {
    number: i32,
    upgrade: i32,
    target: i32,
    pools: Vec<(HandleId, Vitals)>,
    pending: Vec<Unapplied>,
}

pub(in super::super) struct TransferPlan {
    row: usize,
    levels: Vec<(i32, i32)>,
    pools: Vec<(HandleId, Vitals)>,
    research: Vec<Plan>,
    new_records: usize,
}

/// Supported slots and their effects.
type Applied<'a> = Vec<(&'a Slot, Effect)>;
/// Slots left unapplied, with the reason.
type Pending<'a> = Vec<(&'a Slot, &'static str)>;

/// Supported slots among `slots`, and pending ones that would change something.
fn split<'a>(
    table: &crate::upgrade_effects::Table,
    slots: &'a [Slot],
    from: i32,
    to: i32,
) -> (Applied<'a>, Pending<'a>) {
    let (mut applied, mut pending) = (Vec::new(), Vec::new());
    for slot in slots {
        match table.classify(&slot.effect) {
            Effect::Pending { reason, additive } => {
                if !additive || slot.delta(from, to) != 0.0 {
                    pending.push((slot, reason));
                }
            }
            effect => {
                if slot.delta(from, to) != 0.0
                    || (from.min(to) == 0 && from.max(to) > 1 && slot.value(1) != 0.0)
                {
                    applied.push((slot, effect));
                }
            }
        }
    }
    (applied, pending)
}

/// Vitals after applying `slots` for a level change, in slot order.
fn change_pools(
    mut vitals: Vitals,
    applied: &[(&Slot, Effect)],
    from: i32,
    to: i32,
    factor: f32,
    rounding: Option<crate::player_handicap::LifeRounding>,
    dead: bool,
) -> Result<Vitals, String> {
    for (slot, effect) in applied {
        let delta = slot.delta(from, to);
        let name = String::from_utf8_lossy(&slot.effect).into_owned();
        match effect {
            Effect::MaxMana => {
                // The native routine skips a decrease on a unit without maximum mana.
                if delta < 0.0 && vitals.max_mana <= 0.0 {
                    continue;
                }
                (vitals.mana, vitals.max_mana) =
                    change_max_mana(vitals.mana, vitals.max_mana, delta, rounding)
                        .map_err(|e| format!("{name}: {e}"))?;
            }
            Effect::MaxLife => {
                (vitals.life, vitals.max_life) =
                    change_max_life(vitals.life, vitals.max_life, delta, factor, rounding, dead)
                        .map_err(|e| format!("{name}: {e}"))?;
            }
            Effect::ManaRegen => {
                let rate = vitals.regen.mana_per_second + delta;
                if !rate.is_finite() || rate < 0.0 {
                    return Err(format!("{name}: invalid mana regeneration rate"));
                }
                vitals.regen.mana_per_second = rate;
            }
            Effect::Pending { .. } => unreachable!("pending effects are split out"),
        }
    }
    match ObjectHost::projected_pool_problem(&vitals) {
        Some(problem) => Err(problem.into()),
        None => Ok(vitals),
    }
}

impl ObjectHost {
    pub(in super::super) fn project_form_upgrades(
        &self,
        unit: HandleId,
        next: &pcx_sim::world::Definition,
    ) -> Result<Option<Vitals>, Error> {
        let fail = |detail: String| self.inner.error("morph", Failure::UnitActivity(detail));
        let Some(players) = &self.players else {
            return Ok(None);
        };
        let Some(model) = players.upgrade_research.as_ref().filter(|m| m.active) else {
            return Ok(None);
        };
        let record = self.executed_unit(unit)?;
        let Some(row) = players.rows.iter().find(|row| row.identity == record.owner) else {
            return Err(fail("owner research row is missing".into()));
        };
        if row
            .research_levels
            .iter()
            .all(|(_, level)| *level == model.rules.initial)
        {
            return Ok(None);
        }
        let Some(mut vitals) = record.vitals else {
            return Ok(None);
        };
        let old = definition_upgrade_codes(&record.definition).map_err(fail)?;
        let new = definition_upgrade_codes(next).map_err(fail)?;
        if row.research_levels.iter().all(|(id, level)| {
            *level == model.rules.initial
                || (!old.contains(&id.to_be_bytes()) && !new.contains(&id.to_be_bytes()))
        }) {
            return Ok(None);
        }
        let table = model.rules.effects().map_err(fail)?;
        let now = self
            .timer_game_time()
            .ok_or_else(|| fail("upgrade effects require the game clock".into()))?;
        let bonuses = self
            .vitals_bonuses()
            .ok_or_else(|| fail("upgrade effects require admitted vitals".into()))?;
        let mut work = usize::MAX;
        if let Some((life, mana)) = self.project_regen_unit(unit, &bonuses, now, None, &mut work)? {
            vitals.life = life;
            vitals.mana = mana;
            vitals.regen.through = now;
        }
        let dead = record.combat.as_ref().is_some_and(|combat| combat.dead);
        let factor = self.owner_health_handicap(record.owner)?;
        for (codes, undo) in [(&old, true), (&new, false)] {
            for &(upgrade, level) in &row.research_levels {
                if level == model.rules.initial || !codes.contains(&upgrade.to_be_bytes()) {
                    continue;
                }
                let (from, to) = if undo {
                    (level, model.rules.initial)
                } else {
                    (model.rules.initial, level)
                };
                let (applied, _) = split(table, table.slots(upgrade), from, to);
                vitals = change_pools(
                    vitals,
                    &applied,
                    from,
                    to,
                    factor,
                    self.health_rounding(),
                    dead,
                )
                .map_err(fail)?;
            }
        }
        Ok(Some(vitals))
    }

    pub(in super::super) fn prepare_upgrade_transfer(
        &mut self,
        unit: HandleId,
        old_owner: HandleId,
        new_owner: HandleId,
    ) -> Result<Option<TransferPlan>, Error> {
        let fail = |detail: String| {
            self.inner
                .error("SetUnitOwner", Failure::UnitActivity(detail))
        };
        let Some(players) = &self.players else {
            return Ok(None);
        };
        let Some(model) = players.upgrade_research.as_ref().filter(|m| m.active) else {
            return Ok(None);
        };
        let old = players
            .rows
            .iter()
            .find(|row| row.identity == old_owner)
            .ok_or_else(|| fail("old owner research row is missing".into()))?;
        let row = players
            .rows
            .iter()
            .position(|row| row.identity == new_owner)
            .ok_or_else(|| fail("new owner research row is missing".into()))?;
        let new = &players.rows[row];
        if old.research_levels.is_empty() && new.research_levels.is_empty() {
            return Ok(None);
        }
        let record = self.executed_unit(unit)?;
        let codes = definition_upgrade_codes(&record.definition).map_err(fail)?;
        let table = model.rules.effects().map_err(fail)?;
        let initial = model.rules.initial;
        let mut levels = new.research_levels.clone();
        let mut pools = Vec::new();
        let mut research = Vec::new();
        let old_factor = self.owner_health_handicap(old_owner)?;
        let new_factor = self.owner_health_handicap(new_owner)?;
        if old_factor != new_factor {
            return Err(fail(
                "ownership transfer across health handicaps is not qualified".into(),
            ));
        }
        let rounding = self.health_rounding();
        let dead = record.combat.as_ref().is_some_and(|combat| combat.dead);
        let mut vitals = record.vitals;
        if let Some(projected) = vitals.as_mut()
            && (old
                .research_levels
                .iter()
                .any(|(id, level)| *level != initial && codes.contains(&id.to_be_bytes()))
                || new
                    .research_levels
                    .iter()
                    .any(|(id, level)| *level != initial && codes.contains(&id.to_be_bytes())))
        {
            let now = self
                .timer_game_time()
                .ok_or_else(|| fail("upgrade effects require the game clock".into()))?;
            let bonuses = self
                .vitals_bonuses()
                .ok_or_else(|| fail("upgrade effects require admitted vitals".into()))?;
            let mut work = usize::MAX;
            if let Some((life, mana)) =
                self.project_regen_unit(unit, &bonuses, now, None, &mut work)?
            {
                projected.life = life;
                projected.mana = mana;
                projected.regen.through = now;
            }
        }
        for &(upgrade, level) in &old.research_levels {
            if level == initial || !codes.contains(&upgrade.to_be_bytes()) {
                continue;
            }
            let (applied, _) = split(table, table.slots(upgrade), level, initial);
            if let Some(current) = vitals {
                vitals = Some(
                    change_pools(
                        current, &applied, level, initial, old_factor, rounding, dead,
                    )
                    .map_err(fail)?,
                );
            }
        }
        for code in codes.iter().rev() {
            let upgrade = i32::from_be_bytes(*code);
            let old_level = old
                .research_levels
                .iter()
                .find(|(id, _)| *id == upgrade)
                .map_or(initial, |(_, level)| *level);
            let current = levels
                .iter()
                .find(|(id, _)| *id == upgrade)
                .map_or(initial, |(_, level)| *level);
            if old_level <= current {
                continue;
            }
            let (inherit, _) = table.flags(upgrade).ok_or_else(|| {
                fail(format!(
                    "missing upgrade flags for {}",
                    String::from_utf8_lossy(code)
                ))
            })?;
            if !inherit {
                continue;
            }
            let target = model
                .rules
                .target(upgrade, current, old_level)
                .ok_or_else(|| fail("inherited upgrade is not admitted".into()))?;
            if target == current {
                continue;
            }
            let mut plan =
                self.plan_level_change(new.number, new_owner, upgrade, current, target, &pools)?;
            for (id, vitals) in plan.pools.drain(..) {
                if let Some((_, stored)) = pools.iter_mut().find(|(unit, _)| *unit == id) {
                    *stored = vitals
                } else {
                    pools.push((id, vitals))
                }
            }
            if let Some((_, level)) = levels.iter_mut().find(|(id, _)| *id == upgrade) {
                *level = target
            } else {
                levels.push((upgrade, target))
            }
            research.push(plan);
        }
        for &(upgrade, level) in &levels {
            if level == initial || !codes.contains(&upgrade.to_be_bytes()) {
                continue;
            }
            let (applied, _) = split(table, table.slots(upgrade), initial, level);
            if let Some(current) = vitals {
                vitals = Some(
                    change_pools(
                        current, &applied, initial, level, new_factor, rounding, dead,
                    )
                    .map_err(fail)?,
                );
            }
        }
        if let Some(vitals) = vitals {
            pools.push((unit, vitals))
        }
        let new_records = levels.len() - new.research_levels.len();
        let limit = players
            .research
            .as_ref()
            .ok_or_else(|| fail("tech research is not admitted".into()))?;
        if new_records > limit.max_records.saturating_sub(limit.records) {
            return Err(fail("player tech research budget exceeded".into()));
        }
        let players = self.players.as_mut().expect("checked players");
        players.rows[row]
            .research_levels
            .try_reserve(new_records)
            .map_err(|_| self.inner.error("SetUnitOwner", Failure::Capacity))?;
        Ok(Some(TransferPlan {
            row,
            levels,
            pools,
            research,
            new_records,
        }))
    }

    pub(in super::super) fn commit_upgrade_transfer(
        &mut self,
        plan: Option<TransferPlan>,
    ) -> Result<(), Error> {
        let Some(plan) = plan else { return Ok(()) };
        let players = self.players.as_mut().expect("planned players");
        players.rows[plan.row].research_levels = plan.levels;
        players.research.as_mut().expect("planned research").records += plan.new_records;
        for (unit, vitals) in plan.pools {
            match self.inner.entry_mut(unit)? {
                Entry::Unit { record } => record.vitals = Some(vitals),
                _ => unreachable!("planned unit identity"),
            }
        }
        for research in plan.research {
            self.commit_level_change(research)?;
        }
        Ok(())
    }
    /// Effects of researched levels that are not applied, with the reason, by player, upgrade
    /// and slot. A slot absent here was applied or has no effect at the researched level.
    pub fn unapplied_upgrade_effects(&self) -> &[Unapplied] {
        self.players
            .as_ref()
            .and_then(|players| players.upgrade_research.as_ref())
            .map_or(&[], |model| model.unapplied.as_slice())
    }

    /// `SetPlayerTechResearched` on the upgrade model: the level change, then its effects on the
    /// owner's existing units. A refused projection leaves the level and every pool unchanged.
    pub(in super::super) fn set_researched_with_effects(
        &mut self,
        args: &[Value],
    ) -> Result<Value, Error> {
        let [player, Value::Int(upgrade), Value::Int(requested)] = args else {
            return self.apply_player(NATIVE, args);
        };
        let at = self.player_index(player, NATIVE)?;
        let (number, owner, current, target) = {
            let players = self.players.as_ref().expect("checked player rows");
            let model = players
                .upgrade_research
                .as_ref()
                .expect("routed upgrade model");
            let row = &players.rows[at];
            let current = row
                .research_levels
                .iter()
                .find(|(id, _)| id == upgrade)
                .map_or(model.rules.initial, |(_, level)| *level);
            match model
                .rules
                .target(*upgrade, current, *requested)
                .filter(|_| model.active)
            {
                Some(target) if target != current => (row.number, row.identity, current, target),
                // An unchanged level, an unknown upgrade or an invalidated model.
                _ => return self.apply_player(NATIVE, args),
            }
        };
        let plan = self.plan_level_change(number, owner, *upgrade, current, target, &[])?;
        let result = self.apply_player(NATIVE, args)?;
        self.commit_level_change(plan)?;
        Ok(result)
    }

    fn plan_level_change(
        &self,
        number: i32,
        owner: HandleId,
        upgrade: i32,
        from: i32,
        to: i32,
        prior: &[(HandleId, Vitals)],
    ) -> Result<Plan, Error> {
        let fail = |detail: String| self.inner.error(NATIVE, Failure::UnitActivity(detail));
        let model = self
            .players
            .as_ref()
            .and_then(|players| players.upgrade_research.as_ref())
            .expect("routed upgrade model");
        let table = model.rules.effects().map_err(fail)?;
        if table.flags(upgrade).is_some_and(|(_, global)| global) {
            return Err(fail(
                "global upgrade player-level effect routines are not decoded".into(),
            ));
        }
        let (applied, pending) = split(table, table.slots(upgrade), from, to);
        let mut plan = Plan {
            number,
            upgrade,
            target: to,
            pools: Vec::new(),
            pending: pending
                .into_iter()
                .map(|(slot, reason)| Unapplied {
                    player: number,
                    upgrade,
                    slot: slot.number,
                    effect: slot.effect,
                    from,
                    to,
                    reason,
                })
                .collect(),
        };
        if applied.is_empty() {
            return Ok(plan);
        }
        let code = upgrade.to_be_bytes();
        let mut owned = Vec::new();
        for (id, record) in self.executed_units() {
            if record.owner == owner
                && record.vitals.is_some()
                && definition_upgrade_codes(&record.definition)
                    .map_err(fail)?
                    .contains(&code)
            {
                owned.push(id);
            }
        }
        if owned.is_empty() {
            return Ok(plan);
        }
        if !projection_fits(from, to, owned.len(), applied.len()) {
            return Err(fail("upgrade level projection work limit exceeded".into()));
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| fail("upgrade effects require the game clock".into()))?;
        let bonuses = self
            .vitals_bonuses()
            .ok_or_else(|| fail("upgrade effects require admitted vitals".into()))?;
        let rounding = self.health_rounding();
        let factor = self.owner_health_handicap(owner)?;
        let mut work = usize::MAX;
        for id in owned {
            let record = self.executed_unit(id)?;
            let mut vitals = if let Some((_, vitals)) = prior.iter().find(|(unit, _)| *unit == id) {
                *vitals
            } else {
                let mut vitals = record.vitals.expect("filtered to vitals");
                if let Some((life, mana)) =
                    self.project_regen_unit(id, &bonuses, now, None, &mut work)?
                {
                    vitals.life = life;
                    vitals.mana = mana;
                    vitals.regen.through = now;
                }
                vitals
            };
            let dead = record.combat.as_ref().is_some_and(|c| c.dead);
            let mut level = from;
            while level != to {
                let next = if level < to { level + 1 } else { level - 1 };
                vitals = change_pools(vitals, &applied, level, next, factor, rounding, dead)
                    .map_err(|e| fail(format!("{}: {e}", String::from_utf8_lossy(&code))))?;
                level = next;
            }
            plan.pools.push((id, vitals));
        }
        Ok(plan)
    }

    fn commit_level_change(&mut self, plan: Plan) -> Result<(), Error> {
        for (id, vitals) in plan.pools {
            match self.inner.entry_mut(id)? {
                Entry::Unit { record } => record.vitals = Some(vitals),
                _ => unreachable!("planned unit identity"),
            }
        }
        let model = self
            .players
            .as_mut()
            .and_then(|players| players.upgrade_research.as_mut())
            .expect("routed upgrade model");
        model
            .unapplied
            .retain(|entry| !(entry.player == plan.number && entry.upgrade == plan.upgrade));
        if plan.target != model.rules.initial {
            model.unapplied.extend(plan.pending);
        }
        Ok(())
    }

    /// Apply the owner's researched levels to a unit being created: the level change from the
    /// initial level to the current one, for each upgrade in the unit type's list, in list order.
    /// The pools at this point are the creation pools, so the ratio rule starts from them.
    pub(in super::super) fn apply_creation_upgrades(
        &self,
        record: &mut ExecutedUnit,
    ) -> Result<(), Error> {
        let Some(players) = &self.players else {
            return Ok(());
        };
        let Some(model) = players.upgrade_research.as_ref().filter(|m| m.active) else {
            return Ok(());
        };
        let (Some(mut vitals), Some(row)) = (
            record.vitals,
            players.rows.iter().find(|row| row.identity == record.owner),
        ) else {
            return Ok(());
        };
        let initial = model.rules.initial;
        let researched: Vec<(i32, i32)> = row
            .research_levels
            .iter()
            .copied()
            .filter(|(_, level)| *level != initial)
            .collect();
        if researched.is_empty() {
            return Ok(());
        }
        let fail = |detail: String| {
            self.inner
                .error("CreateUnit", Failure::UnitActivity(detail))
        };
        let table = model.rules.effects().map_err(fail)?;
        if researched
            .iter()
            .all(|(upgrade, _)| table.slots(*upgrade).is_empty())
        {
            return Ok(());
        }
        let rounding = self.health_rounding();
        for code in definition_upgrade_codes(&record.definition).map_err(fail)? {
            let upgrade = i32::from_be_bytes(code);
            let Some(&(_, level)) = researched.iter().find(|(id, _)| *id == upgrade) else {
                continue;
            };
            let (applied, _) = split(table, table.slots(upgrade), initial, level);
            vitals = change_pools(
                vitals,
                &applied,
                initial,
                level,
                record.health_handicap,
                rounding,
                false,
            )
            .map_err(|e| fail(format!("{}: {e}", String::from_utf8_lossy(&code))))?;
        }
        record.vitals = Some(vitals);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::projection_fits;

    #[test]
    fn projection_work_is_bounded() {
        assert!(projection_fits(0, 1000, 1000, 1));
        assert!(!projection_fits(0, 1001, 1000, 1));
        assert!(!projection_fits(i32::MIN, i32::MAX, usize::MAX, 2));
    }
}

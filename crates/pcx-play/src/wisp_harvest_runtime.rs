//! Wisp Harvest (`Awha`, `CAbilityWispHarvest`): a wisp on a tree credits its owner with lumber
//! at a fixed interval. There is no carrying, no depot trip and no tree damage. Decoded from the
//! reviewed client in `docs/re-notes/wisp-harvest.md`.
//!
//! Open, so not applied: the arrival repositioning of the wisp onto the tree (`0xbbe300`), the
//! unit flags the handler sets (`+0x48c |= 0x80`), the lumber floating text (`0xb78c20`), and the
//! immediate `autoharvestlumber` order, which this class does not answer.
use super::*;
use crate::harvest_rules::WispRules;

#[derive(Debug, Clone, Copy, PartialEq)]
enum WispPhase {
    ToTree,
    /// Game time of the next credit.
    Harvesting {
        next_credit: f64,
    },
}

#[derive(Debug, Clone)]
pub(super) struct Wisp {
    pub(super) unit: HandleId,
    rules: WispRules,
    tree: HandleId,
    phase: WispPhase,
    paused_at: Option<f64>,
}

impl ObjectHost {
    /// The unit's wisp-harvest rules, `None` when its definition lists no `Awha` ability.
    fn wisp_setup(
        &self,
        definition: &pcx_sim::world::Definition,
    ) -> Result<Option<WispRules>, Error> {
        let state = self.harvest.as_ref().expect("harvest admitted");
        WispRules::from_definition(&state.tables, &state.abilities, definition)
            .map_err(|error| self.harvest_read_error("Awha", error))
    }

    /// A `harvest` or `autoharvestlumber` order for a unit with `Awha`. `Ok(None)` leaves the
    /// order to the peasant harvest path.
    pub(super) fn start_wisp_harvest(
        &mut self,
        unit: HandleId,
        tree: Option<HandleId>,
    ) -> Result<Option<Value>, Error> {
        let Ok(record) = self.executed_unit(unit) else {
            return Ok(None);
        };
        let Some(rules) = self.wisp_setup(&record.definition)? else {
            return Ok(None);
        };
        let Some(tree) = tree else {
            return Err(self.harvest_error(
                "autoharvestlumber for a wisp is not decoded: Awha answers a harvest order on a tree",
            ));
        };
        if !record.shown
            || record.paused_since.is_some()
            || record.combat.as_ref().is_some_and(|combat| combat.dead)
        {
            return Ok(Some(Value::Bool(false)));
        }
        let position = record
            .position
            .ok_or_else(|| self.harvest_error("accepted unit position required"))?;
        if !self.explicit_tree_is_harvestable(tree)? {
            return Ok(Some(Value::Bool(false)));
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.harvest_error("clock not admitted"))?;
        let tree_position = self.tree_position(tree)?;
        let in_reach = distance(position, tree_position) <= rules.range;
        let route = self
            .prepare_steer_unit_in_range(
                unit,
                (!in_reach).then_some(tree_position),
                rules.range,
                ORDER,
            )?
            .ok_or_else(|| self.harvest_error("wisp cannot move to the selected tree"))?;
        let replacing = self
            .harvest
            .as_ref()
            .expect("harvest admitted")
            .wisps
            .iter()
            .any(|wisp| wisp.unit == unit);
        self.reserve_wisp_task(!replacing)?;
        self.interrupt_unit_construction(unit)?;
        self.commit_steer_unit(unit, route, HARVEST_ORDER)?;
        self.clear_combat_target(unit);
        let phase = if in_reach {
            WispPhase::Harvesting {
                next_credit: now + f64::from(rules.interval),
            }
        } else {
            WispPhase::ToTree
        };
        let wisp = Wisp {
            unit,
            rules,
            tree,
            phase,
            paused_at: None,
        };
        let state = self.harvest.as_mut().expect("harvest admitted");
        state.cargo.remove(&unit);
        match state.wisps.iter().position(|current| current.unit == unit) {
            Some(at) => state.wisps[at] = wisp,
            None => state.wisps.push(wisp),
        }
        Ok(Some(Value::Bool(true)))
    }

    /// Wisps share the worker budget.
    fn reserve_wisp_task(&mut self, new_task: bool) -> Result<(), Error> {
        self.reserve_steer_commits(1, HARVEST_ORDER)?;
        let state = self.harvest.as_mut().expect("harvest admitted");
        let added = usize::from(new_task);
        if state
            .workers
            .len()
            .saturating_add(state.wisps.len())
            .saturating_add(added)
            > state.budget.max_workers
        {
            return Err(self.harvest_error("harvest worker budget exhausted"));
        }
        state
            .wisps
            .try_reserve(added)
            .map_err(|_| self.harvest_error("harvest worker capacity"))
    }

    pub(super) fn cancel_wisp(&mut self, unit: HandleId) {
        if let Some(state) = &mut self.harvest {
            state.wisps.retain(|wisp| wisp.unit != unit);
        }
    }

    pub(super) fn advance_wisps(&mut self, now: f64, work: &mut usize) -> Result<(), Error> {
        let units: Vec<_> = match &self.harvest {
            Some(state) => state.wisps.iter().map(|wisp| wisp.unit).collect(),
            None => return Ok(()),
        };
        for unit in units {
            *work = work
                .checked_sub(1)
                .ok_or_else(|| self.harvest_error("harvest work exhausted"))?;
            self.tick_wisp(unit, now, work)?;
        }
        Ok(())
    }

    fn tick_wisp(&mut self, unit: HandleId, now: f64, work: &mut usize) -> Result<(), Error> {
        let Some(mut wisp) = self
            .harvest
            .as_ref()
            .and_then(|state| state.wisps.iter().find(|wisp| wisp.unit == unit))
            .cloned()
        else {
            return Ok(());
        };
        let live = self.executed_unit(unit).ok().filter(|record| {
            record.shown && !record.combat.as_ref().is_some_and(|combat| combat.dead)
        });
        let Some((paused_since, owner, position)) =
            live.map(|record| (record.paused_since, record.owner, record.position))
        else {
            self.cancel_wisp(unit);
            return Ok(());
        };
        let position =
            position.ok_or_else(|| self.harvest_error("accepted unit position required"))?;
        if let Some(paused_at) = paused_since {
            wisp.paused_at.get_or_insert(paused_at);
            self.store_wisp(wisp);
            return Ok(());
        }
        if let Some(paused_at) = wisp.paused_at.take()
            && let WispPhase::Harvesting { next_credit } = &mut wisp.phase
        {
            *next_credit += (now - paused_at).max(0.0);
        }
        // A tree that stops being harvestable ends the work and the wisp takes the nearest other
        // tree within `Rng1` (`0xbbe560` through `0xbbdda0`); with none it goes idle.
        if !self.tree_usable(wisp.tree) {
            return self.retarget_wisp(wisp, position, now);
        }
        match wisp.phase {
            WispPhase::ToTree => {
                let tree_position = self.tree_position(wisp.tree)?;
                if distance(position, tree_position) <= wisp.rules.range {
                    self.stop_unit(unit)?;
                    wisp.phase = WispPhase::Harvesting {
                        next_credit: now + f64::from(wisp.rules.interval),
                    };
                } else if !self.activity_moving(unit)? && self.activity_waypoints_empty(unit)? {
                    self.stop_unit(unit)?;
                    self.cancel_wisp(unit);
                    return Ok(());
                }
            }
            WispPhase::Harvesting { mut next_credit } => {
                while now + 1e-9 >= next_credit {
                    *work = work
                        .checked_sub(1)
                        .ok_or_else(|| self.harvest_error("harvest work exhausted"))?;
                    let credit = self.prepare_lumber_credit_reserving(
                        owner,
                        wisp.rules.lumber_per_interval,
                        0,
                    )?;
                    self.apply_resource_credit(credit);
                    next_credit += f64::from(wisp.rules.interval);
                }
                wisp.phase = WispPhase::Harvesting { next_credit };
            }
        }
        self.store_wisp(wisp);
        Ok(())
    }

    fn retarget_wisp(&mut self, mut wisp: Wisp, position: [f32; 2], now: f64) -> Result<(), Error> {
        let Some(tree) = self.select_tree_within(position, wisp.rules.range)? else {
            self.cancel_wisp(wisp.unit);
            return Ok(());
        };
        let tree_position = self.tree_position(tree)?;
        wisp.tree = tree;
        if distance(position, tree_position) <= wisp.rules.range {
            wisp.phase = WispPhase::Harvesting {
                next_credit: now + f64::from(wisp.rules.interval),
            };
        } else {
            wisp.phase = WispPhase::ToTree;
            if !self.steer_unit_in_range(wisp.unit, Some(tree_position), wisp.rules.range, ORDER)? {
                self.cancel_wisp(wisp.unit);
                return Ok(());
            }
        }
        self.store_wisp(wisp);
        Ok(())
    }

    fn store_wisp(&mut self, wisp: Wisp) {
        let state = self.harvest.as_mut().expect("harvest admitted");
        if let Some(slot) = state
            .wisps
            .iter_mut()
            .find(|existing| existing.unit == wisp.unit)
        {
            *slot = wisp;
        }
    }
}

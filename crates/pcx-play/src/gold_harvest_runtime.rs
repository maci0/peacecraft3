//! Gold work and campaign assignments use shared harvest tasks and prepared movement.
use super::*;

impl ObjectHost {
    pub(super) fn start_auto_gold_harvest(&mut self, unit: HandleId) -> Result<Value, Error> {
        let record = self.executed_unit(unit)?;
        if !record.shown
            || record.paused_since.is_some()
            || record.combat.as_ref().is_some_and(|combat| combat.dead)
        {
            return Ok(Value::Bool(false));
        }
        let position = record
            .position
            .ok_or_else(|| self.harvest_error("accepted worker position required"))?;
        let Some(mine) = self.select_gold_mine(position)? else {
            return Ok(Value::Bool(false));
        };
        self.start_gold_harvest(unit, mine)
    }

    pub(super) fn start_gold_harvest(
        &mut self,
        unit: HandleId,
        mine: HandleId,
    ) -> Result<Value, Error> {
        let Some(prepared) = self.prepare_gold_harvest(unit, mine)? else {
            return Ok(Value::Bool(false));
        };
        self.reserve_harvest_tasks(
            usize::from(
                !self
                    .harvest
                    .as_ref()
                    .expect("harvest admitted")
                    .workers
                    .iter()
                    .any(|task| task.unit == unit),
            ),
            1,
        )?;
        self.publish_harvest_worker(prepared)?;
        Ok(Value::Bool(true))
    }

    pub(super) fn reserve_harvest_tasks(
        &mut self,
        count: usize,
        routes: usize,
    ) -> Result<(), Error> {
        self.reserve_steer_commits(routes, HARVEST_ORDER)?;
        let state = self.harvest.as_mut().expect("harvest admitted");
        if state.workers.len().saturating_add(count) > state.budget.max_workers {
            return Err(self.harvest_error("harvest worker budget exhausted"));
        }
        state
            .workers
            .try_reserve(count)
            .map_err(|_| self.harvest_error("harvest worker capacity"))
    }

    fn prepare_gold_harvest(
        &mut self,
        unit: HandleId,
        mine: HandleId,
    ) -> Result<Option<PreparedHarvest>, Error> {
        let record = self.executed_unit(unit)?;
        if !record.shown
            || record.paused_since.is_some()
            || record.combat.as_ref().is_some_and(|c| c.dead)
        {
            return Ok(None);
        }
        let position = record
            .position
            .ok_or_else(|| self.harvest_error("accepted worker position required"))?;
        let owner = record.owner;
        let Some(setup) = self.harvest_setup(&record.definition, owner)? else {
            return Ok(None);
        };
        if setup.rules.gold_capacity == 0 {
            return Ok(None);
        }
        if !setup.rules.range.is_finite()
            || setup.rules.range < 0.0
            || !setup.rules.search_radius.is_finite()
            || setup.rules.search_radius <= 0.0
        {
            return Err(self.harvest_error(
                "positive gold capacity/search radius and nonnegative harvest range required",
            ));
        }
        let Some((mine_position, mine_rules)) = self.gold_mine(mine)? else {
            return Ok(None);
        };
        // The worker's own area first; failing that, the mine's: the shipped Human04 opening
        // orders a worker 1012 from its only town hall (800 from the mine) to harvest.
        let depot = match self.select_depot(unit, owner, position, &setup.rules, true)? {
            Some(depot) => Some(depot),
            None => self.select_depot(unit, owner, mine_position, &setup.rules, true)?,
        }
        .ok_or_else(|| {
            self.harvest_error("no gold-accepting depot within Ahar Area of the worker or the mine")
        })?;
        let state = self.harvest.as_ref().expect("harvest admitted");
        let existing = state.workers.iter().find(|worker| worker.unit == unit);
        let cargo = state.cargo.get(&unit);
        if existing.is_some_and(|worker| worker.carrying >= 1.0)
            || cargo.is_some_and(|cargo| cargo.lumber >= 1.0)
        {
            return Err(
                self.harvest_error("return carried lumber before changing harvest resource")
            );
        }
        let carrying = existing
            .and_then(|worker| worker.gold.as_ref())
            .or_else(|| cargo.and_then(|cargo| cargo.gold.as_ref()))
            .map_or(0, |gold| gold.carrying);
        let (target, reach, phase) = if carrying > 0 {
            (
                self.unit_position(depot)?,
                self.depot_reach(depot, &setup.rules)?,
                Phase::ToDepot,
            )
        } else {
            (
                mine_position,
                self.unit_collision_radius(mine, HARVEST_ORDER)? + setup.rules.range,
                Phase::ToTree,
            )
        };
        let destination = (distance(position, target) > reach).then_some(target);
        let Some(route) =
            self.prepare_steer_unit_in_range(unit, destination, reach, HARVEST_ORDER)?
        else {
            return Ok(None);
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.harvest_error("clock not admitted"))?;
        Ok(Some(PreparedHarvest {
            worker: Worker {
                unit,
                owner,
                rules: setup.rules,
                base_rules: setup.base_rules,
                upgrade_codes: setup.upgrade_codes,
                tree: Some(mine),
                depot: Some(depot),
                carrying: 0.0,
                gold: Some(Gold {
                    mine,
                    rules: mine_rules,
                    carrying,
                }),
                phase,
                phase_since: now,
                next_hit: 0.0,
                paused_at: None,
                reroute: false,
                manual_return: false,
                initial_order: None,
                ai_town: None,
            },
            route,
            animation: None,
        }))
    }

    pub(super) fn publish_harvest_worker(
        &mut self,
        prepared: PreparedHarvest,
    ) -> Result<(), Error> {
        let PreparedHarvest {
            worker,
            route,
            animation,
        } = prepared;
        self.interrupt_unit_construction(worker.unit)?;
        self.commit_steer_unit(worker.unit, route, HARVEST_ORDER)?;
        if let Some(animation) = animation
            && let Entry::Unit { record } = self.inner.entry_mut(worker.unit)?
        {
            record.animation = Some(animation);
        }
        self.clear_combat_target(worker.unit);
        let state = self.harvest.as_mut().expect("harvest admitted");
        state.cargo.remove(&worker.unit);
        match state
            .workers
            .iter()
            .position(|current| current.unit == worker.unit)
        {
            Some(at) => state.workers[at] = worker,
            None => state.workers.push(worker),
        }
        Ok(())
    }

    fn gold_mine(&self, mine: HandleId) -> Result<Option<([f32; 2], GoldMineRules)>, Error> {
        let Some(position) = self.gold_mine_position(mine, false)? else {
            return Ok(None);
        };
        let record = self.executed_unit(mine)?;
        let state = self.harvest.as_ref().expect("harvest admitted");
        let rules =
            GoldMineRules::from_definition(&state.tables, &state.abilities, &record.definition)
                .map_err(|error| self.harvest_read_error("Agld", error))?;
        Ok(rules.map(|rules| (position, rules)))
    }

    fn gold_mine_position(
        &self,
        mine: HandleId,
        retained: bool,
    ) -> Result<Option<[f32; 2]>, Error> {
        let record = match self.executed_unit(mine) {
            Ok(record) => record,
            Err(_) => return Ok(None),
        };
        if (!retained && !record.shown)
            || record.combat.as_ref().is_some_and(|c| c.dead)
            || !record.resource_amount.is_some_and(|amount| amount > 0)
        {
            return Ok(None);
        }
        record
            .position
            .map(Some)
            .ok_or_else(|| self.harvest_error("accepted mine position required"))
    }

    fn select_gold_mine(&self, from: [f32; 2]) -> Result<Option<HandleId>, Error> {
        let mut best = None;
        let mut work = self
            .harvest
            .as_ref()
            .expect("harvest admitted")
            .budget
            .scan_work;
        for (unit, record) in self.executed_units() {
            work = work
                .checked_sub(1)
                .ok_or_else(|| self.harvest_error("gold mine scan work exhausted"))?;
            if record.resource_amount.is_none() {
                continue;
            }
            let Some((position, _)) = self.gold_mine(unit)? else {
                continue;
            };
            let squared = squared_distance(from, position);
            if best.is_none_or(|(score, id)| (squared, unit) < (score, id)) {
                best = Some((squared, unit));
            }
        }
        Ok(best.map(|(_, unit)| unit))
    }

    pub(super) fn seek_gold(
        &mut self,
        worker: &mut Worker,
        position: [f32; 2],
    ) -> Result<(), Error> {
        let old = worker.gold.as_ref().expect("gold task").mine;
        let mine = if self.gold_mine_position(old, true)?.is_some() {
            Some(old)
        } else {
            self.select_gold_mine(position)?
        };
        let Some(mine) = mine else {
            self.stop_unit(worker.unit)?;
            worker.tree = None;
            worker.phase = Phase::Seek;
            return Ok(());
        };
        let (target, rules) = if mine == old {
            (
                self.gold_mine_position(mine, true)?
                    .expect("selected live mine"),
                worker.gold.as_ref().expect("gold task").rules.clone(),
            )
        } else {
            self.gold_mine(mine)?.expect("selected live mine")
        };
        let reach = self.unit_collision_radius(mine, HARVEST_ORDER)? + worker.rules.range;
        let destination = (distance(position, target) > reach).then_some(target);
        let route = self
            .prepare_steer_unit_in_range(worker.unit, destination, reach, HARVEST_ORDER)?
            .ok_or_else(|| self.harvest_error("mine is unreachable"))?;
        self.reserve_steer_commits(1, HARVEST_ORDER)?;
        self.commit_steer_unit(worker.unit, route, HARVEST_ORDER)?;
        let gold = worker.gold.as_mut().expect("gold task");
        gold.mine = mine;
        gold.rules = rules;
        worker.tree = Some(mine);
        worker.phase = Phase::ToTree;
        Ok(())
    }

    pub(super) fn advance_gold_worker(
        &mut self,
        worker: &mut Worker,
        position: [f32; 2],
        now: f64,
    ) -> Result<(), Error> {
        let gold = worker.gold.as_ref().expect("gold task");
        let mine = gold.mine;
        if worker.phase == Phase::InMine {
            if now < worker.next_hit {
                return Ok(());
            }
            let source = self.executed_unit(mine).map_err(|error| {
                self.harvest_error(format!(
                    "occupied mine removal is not qualified: worker {:?}, mine {:?}: {error}",
                    worker.unit, mine,
                ))
            })?;
            if source.combat.as_ref().is_some_and(|combat| combat.dead) {
                return Err(self.harvest_error(format!(
                    "occupied mine death is not qualified: worker {:?}, mine {:?}",
                    worker.unit, mine,
                )));
            }
            let amount = source
                .resource_amount
                .ok_or_else(|| self.harvest_error("mine resource amount is not admitted"))?
                .max(0)
                .min(worker.rules.gold_capacity);
            let exhausted = amount > 0 && source.resource_amount == Some(amount);
            if exhausted && (self.combat.is_none() || source.combat.is_none()) {
                return Err(
                    self.harvest_error("mine exhaustion needs the admitted unit death runtime")
                );
            }
            let reveal = self.prepare_unit_visibility(worker.unit, true, HARVEST_ORDER)?;
            let from = reveal
                .position
                .ok_or_else(|| self.harvest_error("accepted exit position required"))?;
            let depot = match worker.depot {
                Some(depot) if self.usable_retained_depot(depot, worker.owner, true)? => Some(depot),
                _ => self.select_depot(worker.unit, worker.owner, from, &worker.rules, true)?,
            }.ok_or_else(|| self.harvest_error(format!(
                "no gold-accepting depot at mine exit: worker {:?}, exit {:?}, acquisition radius {}, previous depot {:?}",
                worker.unit, from, worker.rules.search_radius, worker.depot,
            )))?;
            let target = self.unit_position(depot)?;
            let reach = self.depot_reach(depot, &worker.rules)?;
            let route = self
                .prepare_steer_unit_from(
                    worker.unit,
                    Some(from),
                    (distance(from, target) > reach).then_some(target),
                    reach,
                    HARVEST_ORDER,
                )?
                .ok_or_else(|| self.harvest_error("gold depot is unreachable"))?;
            self.reserve_steer_commits(1, HARVEST_ORDER)?;
            self.commit_unit_visibility(worker.unit, reveal, HARVEST_ORDER)?;
            self.commit_steer_unit(worker.unit, route, HARVEST_ORDER)?;
            if let Entry::Unit { record } = self.inner.entry_mut(mine)? {
                *record.resource_amount.as_mut().expect("mine preflighted") -= amount;
            }
            worker.gold.as_mut().expect("gold task").carrying = amount;
            worker.depot = Some(depot);
            worker.phase = Phase::ToDepot;
            worker.reroute = false;
            if exhausted {
                self.kill_unit(mine, None)?;
            }
            return Ok(());
        }
        let Some(target) = self.gold_mine_position(mine, true)? else {
            return self.seek_gold(worker, position);
        };
        let rules = &gold.rules;
        let reach = self.unit_collision_radius(mine, HARVEST_ORDER)? + worker.rules.range;
        if distance(position, target) > reach {
            if !self.activity_moving(worker.unit)? {
                self.seek_gold(worker, position)?;
            }
            return Ok(());
        }
        let inside = self
            .harvest
            .as_ref()
            .expect("harvest admitted")
            .workers
            .iter()
            .filter(|task| {
                task.unit != worker.unit
                    && task.phase == Phase::InMine
                    && task.gold.as_ref().is_some_and(|gold| gold.mine == mine)
            })
            .count();
        let stop = self
            .prepare_steer_unit_in_range(worker.unit, None, 0.0, HARVEST_ORDER)?
            .ok_or_else(|| self.harvest_error("worker cannot stop at mine"))?;
        self.reserve_steer_commits(1, HARVEST_ORDER)?;
        if inside >= rules.max_workers as usize {
            self.commit_steer_unit(worker.unit, stop, HARVEST_ORDER)?;
            return Ok(());
        }
        let deadline = now + f64::from(rules.work_duration);
        if !deadline.is_finite() {
            return Err(self.harvest_error("mining deadline is not finite"));
        }
        let hide = self.prepare_unit_visibility(worker.unit, false, HARVEST_ORDER)?;
        self.commit_unit_visibility(worker.unit, hide, HARVEST_ORDER)?;
        self.commit_steer_unit(worker.unit, stop, HARVEST_ORDER)?;
        worker.phase = Phase::InMine;
        worker.next_hit = deadline;
        Ok(())
    }

    pub(crate) fn harvest_ai_gold(
        &mut self,
        player: i32,
        town: i32,
        count: i32,
    ) -> Result<(), Error> {
        self.harvest_ai_workers(player, town, count, true)
    }

    pub(crate) fn harvest_ai_wood(
        &mut self,
        player: i32,
        town: i32,
        count: i32,
    ) -> Result<(), Error> {
        self.harvest_ai_workers(player, town, count, false)
    }

    fn harvest_ai_workers(
        &mut self,
        player: i32,
        town: i32,
        count: i32,
        gold: bool,
    ) -> Result<(), Error> {
        let name = if gold { "HarvestGold" } else { "HarvestWood" };
        let policy = self.campaign_gathering.as_ref().ok_or_else(|| {
            self.inner.error(
                name,
                Failure::ConfigState("gathering policy is not admitted"),
            )
        })?;
        let slots = policy.harvest_town_slots;
        let index = usize::from(!gold);
        let current = policy.harvest_current_masks[index];
        let other = policy.harvest_current_masks[1 - index];
        let previous = policy.harvest_previous_masks[index];
        let owner = self.player_identity(player).ok_or_else(|| {
            self.inner
                .error(name, Failure::ConfigState("player is not admitted"))
        })?;
        if !self.campaign_ai.is_active(player) {
            return Err(self
                .inner
                .error(name, Failure::ConfigState("AI player is not active")));
        }
        if count <= 0 || town < 0 || town as u32 >= slots {
            return Ok(());
        }
        let state = self.harvest.as_ref().ok_or_else(|| {
            self.inner.error(
                name,
                Failure::ConfigState("harvest runtime is not admitted"),
            )
        })?;
        let budget = state.budget.scan_work;
        let mut units = Vec::new();
        for (unit, record) in self.executed_units() {
            if units.len() >= budget {
                return Err(self.inner.error(name, Failure::Capacity));
            }
            units
                .try_reserve(1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            units.push((
                unit,
                record.owner,
                record.shown && !record.combat.as_ref().is_some_and(|c| c.dead),
                record.definition.rawcode,
            ));
        }
        let mut towns = Vec::new();
        towns
            .try_reserve(units.len())
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        for &(unit, unit_owner, live, rawcode) in &units {
            if unit_owner != owner || !live {
                continue;
            }
            // Both resource calls share town slots; return buildings are selected separately.
            if self
                .depot_return(rawcode)?
                .is_some_and(|depot| depot.accepts_gold)
                && super::super::unit_types::structure(&self.executed_unit(unit)?.definition)
                    .map_err(|error| self.harvest_error(error))?
            {
                towns.push(unit);
            }
        }
        // shortcut: canonical creation order supplies town slots; qualify native town reordering before changing it.
        let Some(&depot) = towns.get(town as usize) else {
            return Ok(());
        };
        let mine = if gold {
            let Some(mine) = self.select_gold_mine(self.unit_position(depot)?)? else {
                return Ok(());
            };
            Some(mine)
        } else {
            None
        };
        let mut candidates = Vec::new();
        candidates
            .try_reserve(units.len())
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        for &(unit, unit_owner, live, _) in &units {
            if unit_owner != owner {
                continue;
            }
            let record = self.executed_unit(unit)?;
            let flags = record.ai_harvest_flags.unwrap_or(0);
            if flags & (current | other) != 0 {
                continue;
            }
            let existing = self
                .harvest
                .as_ref()
                .expect("harvest admitted")
                .workers
                .iter()
                .find(|worker| worker.unit == unit);
            let reuse = existing.is_some_and(|worker| {
                worker.ai_town.is_none_or(|town| town == depot)
                    && match mine {
                        Some(mine) => worker.gold.as_ref().is_some_and(|gold| gold.mine == mine),
                        None => {
                            worker.gold.is_none()
                                && (worker.ai_town == Some(depot) || worker.depot == Some(depot))
                        }
                    }
            });
            if reuse {
                candidates.push((flags & previous == 0, unit, true));
                continue;
            }
            if !live
                || record.paused_since.is_some()
                || existing.is_some()
                || record
                    .activity
                    .as_ref()
                    .is_some_and(|activity| activity.moving())
                || record
                    .combat
                    .as_ref()
                    .is_some_and(|combat| combat.target.is_some())
            {
                continue;
            }
            let state = self.harvest.as_ref().expect("harvest admitted");
            if HarvestRules::from_definition(&state.tables, &state.abilities, &record.definition)
                .map_err(|error| self.harvest_read_error("Ahar", error))?
                .is_some_and(|rules| {
                    if gold {
                        rules.gold_capacity > 0
                    } else {
                        rules.lumber_capacity > 0
                    }
                })
            {
                candidates.push((flags & previous == 0, unit, false));
            }
        }
        candidates.sort_unstable();
        let mut plans = Vec::new();
        let mut reused = Vec::new();
        plans
            .try_reserve(candidates.len())
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        reused
            .try_reserve(candidates.len())
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        for &(_, unit, reuse) in candidates.iter().take(count as usize) {
            if reuse {
                reused.push(unit);
                continue;
            }
            let mut prepared = match mine {
                Some(mine) => self.prepare_gold_harvest(unit, mine)?,
                None => self.prepare_lumber_harvest(unit, None)?,
            }
            .ok_or_else(|| {
                self.inner.error(
                    name,
                    Failure::UnitActivity("selected worker cannot harvest".into()),
                )
            })?;
            if !gold {
                prepared.worker.initial_order = Some(self.order_id("smart").ok_or_else(|| {
                    self.inner
                        .error(name, Failure::ConfigState("smart order is not admitted"))
                })?);
            }
            prepared.worker.ai_town = Some(depot);
            plans.push(prepared);
        }
        self.reserve_harvest_tasks(plans.len(), plans.len())?;
        for prepared in plans {
            let unit = prepared.worker.unit;
            self.publish_harvest_worker(prepared)?;
            reused.push(unit);
        }
        for unit in reused {
            if let Some(worker) = self
                .harvest
                .as_mut()
                .expect("admitted")
                .workers
                .iter_mut()
                .find(|worker| worker.unit == unit)
            {
                worker.ai_town = Some(depot);
            }
            if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                *record.ai_harvest_flags.get_or_insert(0) |= current;
            }
        }
        Ok(())
    }
}

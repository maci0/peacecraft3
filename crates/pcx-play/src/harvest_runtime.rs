//! Auto-harvest lumber runtime: tree selection, travel, source-timed chops,
//! carry accrual, depot return and player credit.
//!
//! Reads `Ahar`, the worker's tree weapon, `Artn` and destructable tree
//! classification through [`crate::harvest_rules`]. Engine-tick deaths queue
//! their registered death events at the next script boundary; player credit is
//! prepared before the tree/carry writes that follow it.
use std::collections::BTreeMap;
use std::sync::Arc;

use pcx_objects::editor::Overlay;
use pcx_objects::editor::defaults::DefaultTables;
use pcx_script::{HandleId, Value};

use super::destructables::DestructableStats;
use crate::harvest_rules::{DepotReturn, HarvestError, HarvestRules};
use crate::order_registry::Registry;

use super::{Entry, Error, Failure, ObjectHost};

pub const ORDER: &str = "autoharvestlumber";

/// Admission limits. These are runtime policy, not authored asset values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub max_workers: usize,
    pub scan_work: usize,
}

impl Budget {
    pub const fn new(max_workers: usize, scan_work: usize) -> Self {
        Self {
            max_workers,
            scan_work,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Seek,
    ToTree,
    Work,
    ToDepot,
}

#[derive(Debug, Clone)]
struct Worker {
    unit: HandleId,
    rules: HarvestRules,
    tree: Option<HandleId>,
    depot: Option<HandleId>,
    carrying: f32,
    phase: Phase,
    phase_since: f64,
    next_hit: f64,
}

pub(super) struct State {
    tables: Arc<DefaultTables>,
    abilities: Overlay,
    harvest_order: i32,
    movement_order: i32,
    budget: Budget,
    workers: Vec<Worker>,
    depot_cache: BTreeMap<[u8; 4], DepotReturn>,
}

impl ObjectHost {
    /// Admit the harvest runtime. Absent registry order or activity rules leave
    /// the native unsupported rather than installing defaults.
    pub fn with_harvest(
        mut self,
        tables: Arc<DefaultTables>,
        abilities: Overlay,
        budget: Budget,
    ) -> Result<Self, Error> {
        let fail = |detail: &str| {
            self.inner
                .error(ORDER, Failure::UnitActivity(detail.into()))
        };
        if self.harvest.is_some() {
            return Err(fail("harvest runtime already admitted"));
        }
        if budget.scan_work == 0 {
            return Err(fail("harvest scan budget must be nonzero"));
        }
        if self.unit_activity.is_none() {
            return Ok(self);
        }
        let Some(harvest_order) = self
            .order_registry
            .as_ref()
            .and_then(|registry| registry.id(ORDER))
        else {
            return Ok(self);
        };
        let Some(movement_order) = self.activity_movement_order() else {
            return Err(fail("movement order not admitted"));
        };
        self.harvest = Some(State {
            tables,
            abilities,
            harvest_order,
            movement_order,
            budget,
            workers: Vec::new(),
            depot_cache: BTreeMap::new(),
        });
        Ok(self)
    }

    /// Order dispatch for auto-harvest lumber. Other orders cancel a running
    /// worker and fall through to the activity native.
    pub(super) fn harvest_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.harvest.as_ref()?;
        if name == "GetUnitCurrentOrder" {
            let [Value::Handle(unit)] = args else {
                return None;
            };
            let state = self.harvest.as_ref()?;
            let worker = state.workers.iter().find(|worker| worker.unit == *unit)?;
            let order = match worker.phase {
                Phase::ToDepot => state.movement_order,
                _ => state.harvest_order,
            };
            return Some(Ok(Value::Int(order)));
        }
        let immediate = matches!(name, "IssueImmediateOrder" | "IssueImmediateOrderById");
        let point = matches!(
            name,
            "IssuePointOrder"
                | "IssuePointOrderById"
                | "IssuePointOrderLoc"
                | "IssuePointOrderByIdLoc"
        );
        let target = matches!(name, "IssueTargetOrder" | "IssueTargetOrderById");
        if !immediate && !point && !target {
            return None;
        }
        let unit = match args.first() {
            Some(Value::Handle(unit)) => *unit,
            _ => return None,
        };
        let by_id = name.ends_with("ById") || name.ends_with("ByIdLoc");
        let order = match args.get(1) {
            Some(Value::Str(text)) if !by_id => self
                .order_registry
                .as_ref()
                .and_then(|registry: &Arc<Registry>| registry.id(text)),
            Some(Value::Int(id)) if by_id => Some(*id),
            _ => return None,
        };
        let harvest_order = self.harvest.as_ref()?.harvest_order;
        if !immediate || order != Some(harvest_order) {
            self.cancel_harvest(unit);
            return None;
        }
        Some(self.start_harvest(unit))
    }

    /// Forget a worker without changing its current motion.
    pub(super) fn cancel_harvest(&mut self, unit: HandleId) {
        if let Some(state) = &mut self.harvest {
            state.workers.retain(|worker| worker.unit != unit);
        }
    }

    fn start_harvest(&mut self, unit: HandleId) -> Result<Value, Error> {
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.harvest_error("clock not admitted"))?;
        let record = self
            .executed_unit(unit)
            .map_err(|_| self.harvest_error("unit handle required"))?;
        if !record.shown
            || record.paused_since.is_some()
            || record.combat.as_ref().is_some_and(|combat| combat.dead)
        {
            return Ok(Value::Bool(false));
        }
        let Some(position) = record.position else {
            return Err(self.harvest_error("accepted unit position required"));
        };
        let definition = record.definition.clone();
        let owner = record.owner;
        let (tables, abilities) = {
            let state = self.harvest.as_ref().expect("admitted");
            (Arc::clone(&state.tables), state.abilities.clone())
        };
        let rules = HarvestRules::from_definition(&tables, &abilities, &definition)
            .map_err(|error| self.harvest_read_error("Ahar", error))?
            .ok_or_else(|| self.harvest_error("unit has no Ahar harvest capability"))?;
        validate_rules(&rules).map_err(|detail| self.harvest_error(detail))?;
        let reach = tree_reach(&rules);
        let tree = self
            .select_tree(position, &rules)?
            .ok_or_else(|| self.harvest_error("no harvestable tree within Ahar Area"))?;
        let tree_position = self.tree_position(tree)?;
        let depot = self
            .select_depot(unit, owner, position, &rules)?
            .ok_or_else(|| self.harvest_error("no lumber-accepting depot within Ahar Area"))?;
        if let Some(state) = &mut self.harvest
            && !state.workers.iter().any(|worker| worker.unit == unit)
        {
            if state.workers.len() >= state.budget.max_workers {
                return Err(self.harvest_error("harvest worker budget exhausted"));
            }
            if state.workers.try_reserve(1).is_err() {
                return Err(self.harvest_error("harvest worker capacity"));
            }
        }
        let distance = distance(position, tree_position);
        let phase = if distance <= reach + 1.0 {
            Phase::Work
        } else {
            if !self.steer_unit_in_range(unit, Some(tree_position), reach, ORDER)?
                && distance > reach + 1.0
            {
                return Err(self.harvest_error("worker cannot move to the selected tree"));
            }
            if distance > reach + 1.0 && self.activity_waypoints_empty(unit)? {
                return Err(self.harvest_error("selected tree is not reachable"));
            }
            Phase::ToTree
        };
        let delay = f64::from(work_delay(&rules));
        let worker = Worker {
            unit,
            rules,
            tree: Some(tree),
            depot: Some(depot),
            carrying: 0.0,
            phase,
            phase_since: now,
            next_hit: now + delay,
        };
        if phase == Phase::Work {
            self.enter_work_animation(unit, now)?;
        }
        if let Some(state) = &mut self.harvest {
            match state
                .workers
                .iter()
                .position(|existing| existing.unit == unit)
            {
                Some(index) => state.workers[index] = worker,
                None => state.workers.push(worker),
            }
        }
        Ok(Value::Bool(true))
    }

    pub(super) fn advance_harvest(&mut self, now: f64, work: &mut usize) -> Result<(), Error> {
        let Some(state) = &self.harvest else {
            return Ok(());
        };
        if state.workers.is_empty() {
            return Ok(());
        }
        let units: Vec<_> = state.workers.iter().map(|worker| worker.unit).collect();
        for unit in units {
            *work = work
                .checked_sub(1)
                .ok_or_else(|| self.harvest_error("harvest work exhausted"))?;
            self.tick_worker(unit, now, work)?;
        }
        Ok(())
    }

    fn tick_worker(&mut self, unit: HandleId, now: f64, work: &mut usize) -> Result<(), Error> {
        let Some(state) = &self.harvest else {
            return Ok(());
        };
        let Some(index) = state.workers.iter().position(|worker| worker.unit == unit) else {
            return Ok(());
        };
        let mut worker = state.workers[index].clone();
        let live = match self.executed_unit(unit) {
            Ok(record) => {
                record.shown
                    && record.paused_since.is_none()
                    && !record.combat.as_ref().is_some_and(|combat| combat.dead)
            }
            Err(_) => false,
        };
        if !live {
            self.harvest
                .as_mut()
                .expect("admitted")
                .workers
                .remove(index);
            return Ok(());
        }
        let position = self
            .executed_unit(unit)?
            .position
            .ok_or_else(|| self.harvest_error("accepted unit position required"))?;
        match worker.phase {
            Phase::Seek => self.seek_tree(&mut worker, position)?,
            Phase::ToTree => self.advance_to_tree(&mut worker, position, now)?,
            Phase::Work => self.advance_work(&mut worker, position, now, work)?,
            Phase::ToDepot => self.advance_to_depot(&mut worker, position, now)?,
        }
        if worker.phase == Phase::Seek && worker.tree.is_none() && worker.carrying < 1.0 {
            let state = self.harvest.as_mut().expect("admitted");
            if let Some(index) = state
                .workers
                .iter()
                .position(|existing| existing.unit == unit)
            {
                state.workers.remove(index);
            }
            return Ok(());
        }
        let state = self.harvest.as_mut().expect("admitted");
        if let Some(slot) = state
            .workers
            .iter_mut()
            .find(|existing| existing.unit == worker.unit)
        {
            *slot = worker;
        }
        Ok(())
    }

    fn seek_tree(&mut self, worker: &mut Worker, position: [f32; 2]) -> Result<(), Error> {
        let Some(tree) = self.select_tree(position, &worker.rules)? else {
            worker.phase = Phase::Seek;
            worker.tree = None;
            return Ok(());
        };
        let tree_position = self.tree_position(tree)?;
        worker.tree = Some(tree);
        worker.phase = Phase::ToTree;
        worker.phase_since = self.timer_game_time().unwrap_or(0.0);
        let reach = tree_reach(&worker.rules);
        if distance(position, tree_position) > reach + 1.0 {
            if !self.steer_unit_in_range(worker.unit, Some(tree_position), reach, ORDER)? {
                return Err(self.harvest_error("worker cannot move to the selected tree"));
            }
            if self.activity_waypoints_empty(worker.unit)? {
                return Err(self.harvest_error("selected tree is not reachable"));
            }
        }
        Ok(())
    }

    fn advance_to_tree(
        &mut self,
        worker: &mut Worker,
        position: [f32; 2],
        now: f64,
    ) -> Result<(), Error> {
        let Some(tree) = worker.tree else {
            worker.phase = Phase::Seek;
            return Ok(());
        };
        let usable = self.tree_usable(tree);
        if !usable {
            if worker.carrying >= 1.0 {
                self.route_depot(worker, position)?;
            } else {
                worker.phase = Phase::Seek;
                worker.tree = None;
                self.seek_tree(worker, position)?;
            }
            return Ok(());
        }
        let tree_position = self.tree_position(tree)?;
        let reach = tree_reach(&worker.rules);
        if distance(position, tree_position) <= reach + 1.0 {
            self.stop_unit(worker.unit)?;
            worker.phase = Phase::Work;
            worker.phase_since = now;
            worker.next_hit = now + f64::from(work_delay(&worker.rules));
            self.enter_work_animation(worker.unit, now)?;
            return Ok(());
        }
        if !self.activity_moving(worker.unit)? && now - worker.phase_since > 0.25 {
            return Err(self.harvest_error("selected tree is not reachable"));
        }
        Ok(())
    }

    fn advance_work(
        &mut self,
        worker: &mut Worker,
        position: [f32; 2],
        now: f64,
        work: &mut usize,
    ) -> Result<(), Error> {
        let Some(tree) = worker.tree else {
            worker.phase = Phase::Seek;
            return Ok(());
        };
        if !self.tree_usable(tree) {
            if worker.carrying >= 1.0 {
                self.route_depot(worker, position)?;
            } else {
                worker.phase = Phase::Seek;
                worker.tree = None;
            }
            return Ok(());
        }
        let tree_position = self.tree_position(tree)?;
        let reach = tree_reach(&worker.rules);
        if distance(position, tree_position) > reach + 1.0 {
            worker.phase = Phase::ToTree;
            worker.phase_since = now;
            self.steer_unit_in_range(worker.unit, Some(tree_position), reach, ORDER)?;
            return Ok(());
        }
        if now + 1e-9 < worker.next_hit {
            return Ok(());
        }
        *work = work
            .checked_sub(1)
            .ok_or_else(|| self.harvest_error("harvest work exhausted"))?;
        let death_triggers = self.death_triggers(tree);
        self.reserve_event_delivery(death_triggers.len())?;
        let life = self.damage_destructable(tree, worker.rules.damage)?;
        let capacity = worker.rules.lumber_capacity as f32;
        worker.carrying = (worker.carrying + worker.rules.damage).min(capacity);
        if life <= 0.0 {
            for trigger in death_triggers {
                self.push_reserved_event(
                    trigger,
                    None,
                    super::EventSubject {
                        destructable: Some(tree),
                        ..Default::default()
                    },
                    None,
                );
            }
            if worker.carrying >= 1.0 {
                self.route_depot(worker, position)?;
            } else {
                worker.phase = Phase::Seek;
                worker.tree = None;
                self.seek_tree(worker, position)?;
            }
            return Ok(());
        }
        if worker.carrying + 1e-3 >= capacity {
            self.route_depot(worker, position)?;
            return Ok(());
        }
        worker.next_hit += f64::from(work_interval(&worker.rules));
        Ok(())
    }

    fn advance_to_depot(
        &mut self,
        worker: &mut Worker,
        position: [f32; 2],
        now: f64,
    ) -> Result<(), Error> {
        let Some(depot) = worker.depot else {
            return self.route_depot(worker, position);
        };
        let usable = self
            .executed_unit(depot)
            .is_ok_and(|record| record.shown && !record.combat.as_ref().is_some_and(|c| c.dead));
        if !usable {
            worker.depot = None;
            return self.route_depot(worker, position);
        }
        let depot_position = self.unit_position(depot)?;
        let reach = self.depot_reach(depot, &worker.rules)?;
        if distance(position, depot_position) > reach + 1.0 {
            if !self.activity_moving(worker.unit)? && now - worker.phase_since > 0.25 {
                return Err(self.harvest_error("selected depot is not reachable"));
            }
            return Ok(());
        }
        self.stop_unit(worker.unit)?;
        let amount = worker.carrying.floor() as i32;
        if amount > 0 {
            let owner = self.executed_unit(worker.unit)?.owner;
            let credit = self.prepare_lumber_credit_reserving(owner, amount, 0)?;
            self.apply_lumber_credit(credit);
        }
        worker.carrying = 0.0;
        worker.phase = Phase::Seek;
        worker.tree = None;
        Ok(())
    }

    fn route_depot(&mut self, worker: &mut Worker, position: [f32; 2]) -> Result<(), Error> {
        let owner = self.executed_unit(worker.unit)?.owner;
        let Some(depot) = self.select_depot(worker.unit, owner, position, &worker.rules)? else {
            return Err(self.harvest_error("no lumber-accepting depot within Ahar Area"));
        };
        let depot_position = self.unit_position(depot)?;
        let reach = self.depot_reach(depot, &worker.rules)?;
        worker.depot = Some(depot);
        worker.phase = Phase::ToDepot;
        worker.phase_since = self.timer_game_time().unwrap_or(0.0);
        if distance(position, depot_position) > reach + 1.0 {
            if !self.steer_unit_in_range(worker.unit, Some(depot_position), reach, ORDER)? {
                return Err(self.harvest_error("worker cannot move to the selected depot"));
            }
            if self.activity_waypoints_empty(worker.unit)? {
                return Err(self.harvest_error("selected depot is not reachable"));
            }
        }
        Ok(())
    }

    fn select_tree(
        &mut self,
        from: [f32; 2],
        rules: &HarvestRules,
    ) -> Result<Option<HandleId>, Error> {
        let budget = self.harvest.as_ref().expect("admitted").budget.scan_work;
        let mut work = budget;
        let radius = rules.search_radius;
        let mut best: Option<(f32, HandleId)> = None;
        let mut candidates = Vec::new();
        candidates
            .try_reserve(budget.min(64))
            .map_err(|_| self.harvest_error("harvest tree scan capacity"))?;
        for (handle, creation, state) in self.destructables_with_handles() {
            if candidates.len() >= budget {
                return Err(self.harvest_error("harvest tree scan work exhausted"));
            }
            if candidates.len() == candidates.capacity() {
                let extra = (budget - candidates.len()).min(64);
                candidates
                    .try_reserve(extra)
                    .map_err(|_| self.harvest_error("harvest tree scan capacity"))?;
            }
            candidates.push((handle, creation.objectid, state));
        }
        for (handle, objectid, state) in candidates {
            work = work
                .checked_sub(1)
                .ok_or_else(|| self.harvest_error("harvest tree scan work exhausted"))?;
            if state.dead
                || state.hidden
                || state.invulnerable
                || !state.life.is_some_and(|life| life > 0.0)
            {
                continue;
            }
            let Some(position) = self.tree_position(handle).ok() else {
                continue;
            };
            let squared = squared_distance(from, position);
            if squared > radius * radius {
                continue;
            }
            let stats = self
                .tree_stats(objectid)
                .map_err(|error| self.harvest_error(error))?;
            let Some(stats) = stats else {
                continue;
            };
            if !stats.tree {
                continue;
            }
            if best.is_none_or(|(distance, best_handle)| {
                squared < distance || (squared == distance && handle < best_handle)
            }) {
                best = Some((squared, handle));
            }
        }
        Ok(best.map(|(_, handle)| handle))
    }

    fn select_depot(
        &mut self,
        unit: HandleId,
        owner: HandleId,
        from: [f32; 2],
        rules: &HarvestRules,
    ) -> Result<Option<HandleId>, Error> {
        let budget = self.harvest.as_ref().expect("admitted").budget.scan_work;
        let mut work = budget;
        let radius = rules.search_radius;
        let mut candidates = Vec::new();
        candidates
            .try_reserve(budget.min(64))
            .map_err(|_| self.harvest_error("harvest depot scan capacity"))?;
        for (id, record) in self.executed_units() {
            if candidates.len() >= budget {
                return Err(self.harvest_error("harvest depot scan work exhausted"));
            }
            if candidates.len() == candidates.capacity() {
                let extra = (budget - candidates.len()).min(64);
                candidates
                    .try_reserve(extra)
                    .map_err(|_| self.harvest_error("harvest depot scan capacity"))?;
            }
            candidates.push((id, record.definition.rawcode));
        }
        let mut best: Option<(f32, HandleId)> = None;
        for (id, rawcode) in candidates {
            work = work
                .checked_sub(1)
                .ok_or_else(|| self.harvest_error("harvest depot scan work exhausted"))?;
            if id == unit {
                continue;
            }
            let Ok(record) = self.executed_unit(id) else {
                continue;
            };
            if record.owner != owner
                || !record.shown
                || record.combat.as_ref().is_some_and(|combat| combat.dead)
            {
                continue;
            }
            let position = record.position;
            let Some(position) = position else {
                continue;
            };
            let squared = squared_distance(from, position);
            if squared > radius * radius {
                continue;
            }
            let Some(depot) = self.depot_return(rawcode)? else {
                continue;
            };
            if !depot.accepts_lumber {
                continue;
            }
            if best.is_none_or(|(distance, best_handle)| {
                squared < distance || (squared == distance && id < best_handle)
            }) {
                best = Some((squared, id));
            }
        }
        Ok(best.map(|(_, handle)| handle))
    }

    fn depot_return(&mut self, rawcode: [u8; 4]) -> Result<Option<DepotReturn>, Error> {
        if let Some(cached) = self
            .harvest
            .as_ref()
            .and_then(|state| state.depot_cache.get(&rawcode))
        {
            return Ok(Some(cached.clone()));
        }
        let (tables, abilities) = {
            let state = self.harvest.as_ref().expect("admitted");
            (Arc::clone(&state.tables), state.abilities.clone())
        };
        let Some(record) = self
            .executed_units()
            .find(|(_, record)| record.definition.rawcode == rawcode)
        else {
            return Ok(None);
        };
        let definition = record.1.definition.clone();
        if definition.kind != pcx_objects::editor::ObjectKind::Unit {
            return Ok(None);
        }
        let depot = DepotReturn::from_definition(&tables, &abilities, &definition)
            .map_err(|error| self.harvest_read_error("Artn", error))?;
        if let Some(state) = &mut self.harvest {
            state
                .depot_cache
                .entry(rawcode)
                .or_insert_with(|| depot.clone());
        }
        Ok(Some(depot))
    }

    fn tree_stats(&self, objectid: i32) -> Result<Option<DestructableStats>, String> {
        self.destructable_catalog
            .as_ref()
            .ok_or_else(|| "destructable catalog not admitted".to_string())?
            .stats_if_destructable(objectid)
    }

    fn tree_usable(&self, tree: HandleId) -> bool {
        self.destructable_state(tree).is_ok_and(|state| {
            !state.dead
                && !state.hidden
                && !state.invulnerable
                && state.life.is_some_and(|life| life > 0.0)
        })
    }

    fn tree_position(&self, tree: HandleId) -> Result<[f32; 2], Error> {
        let creation = self
            .destructable_creation(tree)
            .map_err(|_| self.harvest_error("tree handle required"))?;
        Ok([creation.x, creation.y])
    }

    fn unit_position(&self, unit: HandleId) -> Result<[f32; 2], Error> {
        self.executed_unit(unit)?
            .position
            .ok_or_else(|| self.harvest_error("accepted position required"))
    }

    fn depot_reach(&self, depot: HandleId, rules: &HarvestRules) -> Result<f32, Error> {
        Ok(self.unit_collision_radius(depot, ORDER)? + rules.range)
    }

    fn stop_unit(&mut self, unit: HandleId) -> Result<(), Error> {
        self.steer_unit_in_range(unit, None, 0.0, ORDER)?;
        Ok(())
    }

    fn enter_work_animation(&mut self, unit: HandleId, now: f64) -> Result<(), Error> {
        let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
            return Err(self.harvest_error("unit handle required"));
        };
        let Some(animation) = &mut record.animation else {
            return Ok(());
        };
        if !animation.has_sequence("stand work lumber") {
            return Err(self.harvest_error("worker lacks the stand work lumber sequence"));
        }
        animation
            .select("stand work lumber", now, None)
            .map_err(|detail| self.harvest_error(detail))?;
        Ok(())
    }

    fn death_triggers(&self, tree: HandleId) -> Vec<HandleId> {
        self.inner
            .arena
            .iter()
            .filter_map(|entry| match entry {
                Entry::RegisteredDeathEvent(record) if record.widget == tree => {
                    Some(record.trigger)
                }
                _ => None,
            })
            .collect()
    }

    fn harvest_error(&self, detail: impl Into<String>) -> Error {
        self.inner
            .error(ORDER, Failure::UnitActivity(detail.into()))
    }

    fn harvest_read_error(&self, member: &str, error: HarvestError) -> Error {
        self.harvest_error(format!("{member} source read: {error:?}"))
    }
}

fn validate_rules(rules: &HarvestRules) -> Result<(), &'static str> {
    if !rules.damage.is_finite() || rules.damage <= 0.0 {
        return Err("Ahar damage must be finite and positive");
    }
    if rules.lumber_capacity <= 0 {
        return Err("Ahar lumber capacity must be positive");
    }
    if !rules.search_radius.is_finite() || rules.search_radius <= 0.0 {
        return Err("Ahar search radius must be finite and positive");
    }
    if !rules.range.is_finite() || rules.range < 0.0 {
        return Err("Ahar range must be finite and nonnegative");
    }
    if work_interval(rules) <= 0.0 || !work_interval(rules).is_finite() {
        return Err("harvest work interval must be finite and positive");
    }
    if let Some(weapon) = &rules.work_weapon {
        if !weapon.range.is_finite() || weapon.range < 0.0 {
            return Err("tree weapon range must be finite and nonnegative");
        }
        if !weapon.damage_point.is_finite() || weapon.damage_point < 0.0 {
            return Err("tree weapon damage point must be finite and nonnegative");
        }
    }
    Ok(())
}

fn tree_reach(rules: &HarvestRules) -> f32 {
    rules
        .work_weapon
        .as_ref()
        .map_or(rules.range, |weapon| weapon.range)
}

fn work_interval(rules: &HarvestRules) -> f32 {
    rules
        .work_weapon
        .as_ref()
        .map_or(rules.work_duration, |weapon| weapon.cool)
}

fn work_delay(rules: &HarvestRules) -> f32 {
    rules
        .work_weapon
        .as_ref()
        .map_or(0.0, |weapon| weapon.damage_point)
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    squared_distance(a, b).sqrt()
}

fn squared_distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reach_prefers_the_tree_weapon() {
        let rules = HarvestRules {
            source: "synthetic".into(),
            ahar_alias: "Ahar".into(),
            damage: 7.0,
            lumber_capacity: 10,
            gold_capacity: 0,
            search_radius: 500.0,
            range: 200.0,
            work_duration: 9.0,
            work_weapon: Some(crate::harvest_rules::WorkWeapon {
                slot: 1,
                cool: 2.0,
                damage_point: 0.5,
                backswing: 0.25,
                range: 70.0,
            }),
        };
        assert_eq!(tree_reach(&rules), 70.0);
        assert_eq!(work_interval(&rules), 2.0);
        assert_eq!(work_delay(&rules), 0.5);
    }
}

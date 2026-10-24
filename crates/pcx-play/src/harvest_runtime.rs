//! Shared harvest runtime: tree/mine selection, travel, source-timed work,
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
use crate::harvest_rules::{
    DepotReturn, GoldMineRules, HarvestError, HarvestRules, definition_upgrade_codes,
};
use crate::order_registry::Registry;

use super::{Entry, Error, Failure, ObjectHost};

pub const ORDER: &str = "autoharvestlumber";
const HARVEST_ORDER: &str = "harvest";
const RESUME_ORDER: &str = "resumeharvesting";
const RETURN_ORDER: &str = "returnresources";

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
    InMine,
    ToDepot,
}

#[derive(Debug, Clone)]
struct Worker {
    unit: HandleId,
    owner: HandleId,
    rules: HarvestRules,
    base_rules: HarvestRules,
    upgrade_codes: Vec<String>,
    tree: Option<HandleId>,
    depot: Option<HandleId>,
    carrying: f32,
    gold: Option<Gold>,
    phase: Phase,
    phase_since: f64,
    next_hit: f64,
    paused_at: Option<f64>,
    reroute: bool,
    manual_return: bool,
}

#[derive(Debug, Clone)]
struct Gold {
    mine: HandleId,
    rules: GoldMineRules,
    carrying: i32,
}

#[derive(Debug, Clone)]
struct Cargo {
    lumber: f32,
    gold: Option<Gold>,
}

struct HarvestSetup {
    base_rules: HarvestRules,
    rules: HarvestRules,
    upgrade_codes: Vec<String>,
}

pub(super) struct State {
    tables: Arc<DefaultTables>,
    abilities: Overlay,
    upgrades: Overlay,
    autoharvestlumber_order: i32,
    harvest_order: i32,
    resume_order: i32,
    return_order: i32,
    budget: Budget,
    workers: Vec<Worker>,
    cargo: BTreeMap<HandleId, Cargo>,
    depot_cache: BTreeMap<[u8; 4], DepotReturn>,
}

#[path = "gold_harvest_runtime.rs"]
mod gold;

impl Worker {
    fn carrying_any(&self) -> bool {
        self.carrying >= 1.0 || self.gold.as_ref().is_some_and(|gold| gold.carrying > 0)
    }
}

impl ObjectHost {
    /// Admit the harvest runtime. Absent registry order or activity rules leave
    /// the native unsupported rather than installing defaults.
    pub fn with_harvest(
        mut self,
        tables: Arc<DefaultTables>,
        abilities: Overlay,
        upgrades: Overlay,
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
        let Some(autoharvestlumber_order) = self.order_id(ORDER) else {
            return Ok(self);
        };
        let Some(harvest_order) = self.order_id(HARVEST_ORDER) else {
            return Ok(self);
        };
        let Some(resume_order) = self.order_id(RESUME_ORDER) else {
            return Ok(self);
        };
        let Some(return_order) = self.order_id(RETURN_ORDER) else {
            return Ok(self);
        };
        self.harvest = Some(State {
            tables,
            abilities,
            upgrades,
            autoharvestlumber_order,
            harvest_order,
            resume_order,
            return_order,
            budget,
            workers: Vec::new(),
            cargo: BTreeMap::new(),
            depot_cache: BTreeMap::new(),
        });
        Ok(self)
    }

    /// Order dispatch for lumber and gold harvesting. Other orders cancel a running
    /// worker and fall through to the activity native.
    pub(super) fn harvest_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.harvest.as_ref()?;
        self.inner.program.natives().get(name)?;
        if name == "GetUnitCurrentOrder" {
            let [Value::Handle(unit)] = args else {
                return None;
            };
            let state = self.harvest.as_ref()?;
            let worker = state.workers.iter().find(|worker| worker.unit == *unit)?;
            let order = if worker.manual_return {
                state.return_order
            } else {
                match worker.phase {
                    Phase::ToDepot => state.resume_order,
                    _ => state.harvest_order,
                }
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
        if self
            .harvest
            .as_ref()?
            .workers
            .iter()
            .any(|worker| worker.unit == unit && worker.phase == Phase::InMine)
        {
            let valid = if immediate {
                args.len() == 2
            } else if target {
                args.len() == 3 && matches!(args[2], Value::Handle(_) | Value::Null)
            } else if name.contains("Loc") {
                args.len() == 3 && matches!(args[2], Value::Handle(_) | Value::Null)
            } else {
                args.len() == 4
                    && args[2].as_real().is_some_and(f32::is_finite)
                    && args[3].as_real().is_some_and(f32::is_finite)
            };
            return Some(if valid {
                Ok(Value::Bool(false))
            } else {
                Err(self.inner.error(name, Failure::Signature))
            });
        }
        if target && order == Some(self.harvest.as_ref()?.harvest_order) {
            let [_, _, Value::Handle(mine)] = args else {
                return None;
            };
            if self
                .executed_unit(*mine)
                .is_ok_and(|record| record.resource_amount.is_some())
            {
                return Some(self.start_gold_harvest(unit, *mine));
            }
        }
        if !immediate || args.len() != 2 {
            return None;
        }
        let state = self.harvest.as_ref()?;
        if order == Some(state.autoharvestlumber_order) {
            return Some(self.start_harvest(unit));
        }
        if order == Some(state.return_order) {
            return Some(self.return_resources(unit));
        }
        None
    }

    /// `returnresources` accepted for a worker that already carries resources,
    /// including a worker stopped or moved away from its prior harvest.
    /// Explicit return reports `returnresources` and ends idle after credit;
    /// automatic capacity return reports `resumeharvesting` and resumes.
    fn return_resources(&mut self, unit: HandleId) -> Result<Value, Error> {
        let record = match self.executed_unit(unit) {
            Ok(record) => record,
            Err(_) => return Ok(Value::Bool(false)),
        };
        if !record.shown
            || record.paused_since.is_some()
            || record.combat.as_ref().is_some_and(|combat| combat.dead)
        {
            return Ok(Value::Bool(false));
        }
        let Some(position) = record.position else {
            return Err(self.harvest_error("accepted unit position required"));
        };
        let owner = record.owner;
        let existing = {
            let Some(state) = &self.harvest else {
                return Ok(Value::Bool(false));
            };
            state
                .workers
                .iter()
                .position(|worker| worker.unit == unit)
                .filter(|index| state.workers[*index].carrying_any())
        };
        if let Some(index) = existing {
            let mut worker = self.harvest.as_ref().expect("admitted").workers[index].clone();
            if worker.gold.is_none() {
                self.slow_chopping_for(unit)?;
            }
            worker.owner = owner;
            worker.manual_return = true;
            self.route_depot(&mut worker, position)?;
            self.harvest.as_mut().expect("admitted").workers[index] = worker;
            return Ok(Value::Bool(true));
        }
        let cargo = {
            let Some(state) = &self.harvest else {
                return Ok(Value::Bool(false));
            };
            let Some(carrying) = state.cargo.get(&unit).cloned().filter(|cargo| {
                cargo.lumber >= 1.0 || cargo.gold.as_ref().is_some_and(|gold| gold.carrying > 0)
            }) else {
                return Ok(Value::Bool(false));
            };
            carrying
        };
        if cargo.gold.is_none() {
            self.slow_chopping_for(unit)?;
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.harvest_error("clock not admitted"))?;
        let setup = self
            .harvest_setup(&record.definition, owner)?
            .ok_or_else(|| self.harvest_error("unit has no Ahar harvest capability"))?;
        if cargo.gold.is_none() {
            validate_rules(&setup.rules).map_err(|detail| self.harvest_error(detail))?;
        }
        if let Some(state) = &mut self.harvest {
            if state.workers.len() >= state.budget.max_workers {
                return Err(self.harvest_error("harvest worker budget exhausted"));
            }
            if state.workers.try_reserve(1).is_err() {
                return Err(self.harvest_error("harvest worker capacity"));
            }
        }
        let mut worker = Worker {
            unit,
            owner,
            rules: setup.rules,
            base_rules: setup.base_rules,
            upgrade_codes: setup.upgrade_codes,
            tree: None,
            depot: None,
            carrying: cargo.lumber,
            gold: cargo.gold,
            phase: Phase::ToDepot,
            phase_since: now,
            next_hit: 0.0,
            paused_at: None,
            reroute: true,
            manual_return: true,
        };
        self.route_depot(&mut worker, position)?;
        if let Some(state) = &mut self.harvest {
            state.cargo.remove(&unit);
            state.workers.push(worker);
        }
        Ok(Value::Bool(true))
    }

    /// Forget a worker without changing its current motion. Partial lumber
    /// stays attached to the unit and survives stop, move and nearby reissue.
    pub(super) fn cancel_harvest(&mut self, unit: HandleId) {
        if let Some(state) = &mut self.harvest {
            let Some(index) = state.workers.iter().position(|worker| worker.unit == unit) else {
                return;
            };
            let worker = state.workers.remove(index);
            if worker.carrying > 0.0 || worker.gold.as_ref().is_some_and(|gold| gold.carrying > 0) {
                state
                    .cargo
                    .entry(unit)
                    .and_modify(|cargo| {
                        cargo.lumber = cargo.lumber.max(worker.carrying);
                        if worker.gold.is_some() {
                            cargo.gold = worker.gold.clone();
                        }
                    })
                    .or_insert(Cargo {
                        lumber: worker.carrying,
                        gold: worker.gold,
                    });
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn harvest_cargo(&self, unit: HandleId) -> Option<f32> {
        self.harvest
            .as_ref()?
            .cargo
            .get(&unit)
            .map(|cargo| cargo.lumber)
    }

    #[cfg(test)]
    pub(crate) fn harvest_worker_count(&self, unit: HandleId) -> usize {
        self.harvest.as_ref().map_or(0, |state| {
            state
                .workers
                .iter()
                .filter(|worker| worker.unit == unit)
                .count()
        })
    }

    #[cfg(test)]
    pub(crate) fn fixture_harvest_definition(
        &mut self,
        unit: HandleId,
        definition: pcx_sim::world::Definition,
        amount: Option<i32>,
    ) {
        let Entry::Unit { record } = self.inner.entry_mut(unit).expect("fixture unit") else {
            panic!("unit fixture");
        };
        record.definition = definition;
        record.resource_amount = amount;
    }

    pub(super) fn harvest_owns_unit(&self, unit: HandleId) -> bool {
        self.harvest
            .as_ref()
            .is_some_and(|state| state.workers.iter().any(|worker| worker.unit == unit))
    }

    pub(super) fn purge_harvest_cargo(&mut self, unit: HandleId) {
        if let Some(state) = &mut self.harvest {
            state.cargo.remove(&unit);
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
        if self.harvest.as_ref().is_some_and(|state| {
            state.workers.iter().any(|worker| {
                worker.unit == unit && worker.gold.as_ref().is_some_and(|gold| gold.carrying > 0)
            }) || state
                .cargo
                .get(&unit)
                .is_some_and(|cargo| cargo.gold.as_ref().is_some_and(|gold| gold.carrying > 0))
        }) {
            return Err(self.harvest_error("return carried gold before changing harvest resource"));
        }
        self.slow_chopping_for(unit)?;
        self.require_work_animation(unit)?;
        let setup = self
            .harvest_setup(&definition, owner)?
            .ok_or_else(|| self.harvest_error("unit has no Ahar harvest capability"))?;
        let rules = setup.rules;
        validate_rules(&rules).map_err(|detail| self.harvest_error(detail))?;
        let reach = tree_reach(&rules);
        let tree = self
            .select_tree(position, &rules)?
            .ok_or_else(|| self.harvest_error("no harvestable tree within Ahar Area"))?;
        let tree_position = self.tree_position(tree)?;
        let depot = self
            .select_depot(unit, owner, position, &rules, false)?
            .ok_or_else(|| self.harvest_error("no lumber-accepting depot within Ahar Area"))?;
        let carrying = {
            let state = self.harvest.as_ref().expect("admitted");
            state
                .workers
                .iter()
                .find(|worker| worker.unit == unit)
                .map_or_else(
                    || state.cargo.get(&unit).map_or(0.0, |cargo| cargo.lumber),
                    |worker| worker.carrying,
                )
        };
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
        let phase = if distance <= reach {
            Phase::Work
        } else {
            if !self.steer_unit_in_range(unit, Some(tree_position), reach, ORDER)?
                && distance > reach
            {
                return Err(self.harvest_error("worker cannot move to the selected tree"));
            }
            if distance > reach && self.activity_waypoints_empty(unit)? {
                return Err(self.harvest_error("selected tree is not reachable"));
            }
            Phase::ToTree
        };
        let delay = f64::from(work_delay(&rules));
        let worker = Worker {
            unit,
            owner,
            rules,
            base_rules: setup.base_rules,
            upgrade_codes: setup.upgrade_codes,
            tree: Some(tree),
            depot: Some(depot),
            carrying,
            gold: None,
            phase,
            phase_since: now,
            next_hit: now + delay,
            paused_at: None,
            reroute: false,
            manual_return: false,
        };
        if phase == Phase::Work {
            self.enter_work_animation(unit, now)?;
        }
        if let Some(state) = &mut self.harvest {
            state.cargo.remove(&unit);
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
        let record = match self.executed_unit(unit) {
            Ok(record) if !record.combat.as_ref().is_some_and(|combat| combat.dead) => record,
            _ => {
                self.harvest
                    .as_mut()
                    .expect("admitted")
                    .workers
                    .remove(index);
                return Ok(());
            }
        };
        if !record.shown && worker.phase != Phase::InMine {
            self.cancel_harvest(unit);
            return Ok(());
        }
        if let Some(paused_at) = record.paused_since {
            worker.paused_at.get_or_insert(paused_at);
            let state = self.harvest.as_mut().expect("admitted");
            if let Some(slot) = state
                .workers
                .iter_mut()
                .find(|existing| existing.unit == unit)
            {
                *slot = worker;
            }
            return Ok(());
        }
        if let Some(paused_at) = worker.paused_at.take() {
            let frozen = (now - paused_at).max(0.0);
            worker.phase_since += frozen;
            worker.next_hit += frozen;
        }
        let position = record
            .position
            .ok_or_else(|| self.harvest_error("accepted unit position required"))?;
        worker.owner = record.owner;
        if worker.gold.is_none() && !worker.upgrade_codes.is_empty() {
            let (tables, upgrades) = {
                let state = self.harvest.as_ref().expect("admitted");
                (Arc::clone(&state.tables), state.upgrades.clone())
            };
            let rules = self.apply_upgrades(
                &tables,
                &upgrades,
                &record.definition,
                record.owner,
                &worker.base_rules,
            )?;
            validate_rules(&rules).map_err(|detail| self.harvest_error(detail))?;
            if rules != worker.rules {
                worker.rules = rules;
            }
        }
        let result = if worker.gold.is_some() && worker.phase != Phase::ToDepot {
            self.advance_gold_worker(&mut worker, position, now)
        } else {
            match worker.phase {
                Phase::Seek => self.seek_tree(&mut worker, position),
                Phase::ToTree => self.advance_to_tree(&mut worker, position, now),
                Phase::Work => self.advance_work(&mut worker, position, now, work),
                Phase::ToDepot => self.advance_to_depot(&mut worker, position, now),
                Phase::InMine => unreachable!("gold mine phase has gold state"),
            }
        };
        if result.is_err() {
            self.store_worker(worker);
            return result;
        }
        if worker.phase == Phase::Seek
            && worker.tree.is_none()
            && worker.carrying < 1.0
            && worker.gold.as_ref().is_none_or(|gold| gold.carrying == 0)
        {
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
        self.store_worker(worker);
        Ok(())
    }

    fn store_worker(&mut self, worker: Worker) {
        let state = self.harvest.as_mut().expect("admitted");
        if let Some(slot) = state
            .workers
            .iter_mut()
            .find(|existing| existing.unit == worker.unit)
        {
            *slot = worker;
        }
    }

    fn seek_tree(&mut self, worker: &mut Worker, position: [f32; 2]) -> Result<(), Error> {
        let Some(tree) = self.select_tree(position, &worker.rules)? else {
            worker.phase = Phase::Seek;
            worker.tree = None;
            if worker.carrying >= 1.0 {
                self.route_depot(worker, position)?;
            }
            return Ok(());
        };
        let tree_position = self.tree_position(tree)?;
        worker.tree = Some(tree);
        worker.phase = Phase::ToTree;
        worker.phase_since = self.timer_game_time().unwrap_or(0.0);
        let reach = tree_reach(&worker.rules);
        if distance(position, tree_position) > reach {
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
        if distance(position, tree_position) <= reach {
            self.stop_unit(worker.unit)?;
            worker.phase = Phase::Work;
            worker.phase_since = now;
            worker.next_hit = now + f64::from(work_delay(&worker.rules));
            self.enter_work_animation(worker.unit, now)?;
            return Ok(());
        }
        if !self.activity_moving(worker.unit)? && self.activity_waypoints_empty(worker.unit)? {
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
        loop {
            let Some(tree) = worker.tree else {
                worker.phase = Phase::Seek;
                return Ok(());
            };
            if !self.tree_usable(tree) {
                if worker.carrying >= 1.0 {
                    worker.phase = Phase::ToDepot;
                    worker.depot = None;
                    worker.reroute = true;
                    worker.phase_since = now;
                } else {
                    worker.phase = Phase::Seek;
                    worker.tree = None;
                    self.seek_tree(worker, position)?;
                }
                return Ok(());
            }
            let tree_position = self.tree_position(tree)?;
            let reach = tree_reach(&worker.rules);
            if distance(position, tree_position) > reach {
                worker.phase = Phase::ToTree;
                worker.phase_since = now;
                worker.reroute = false;
                self.steer_unit_in_range(worker.unit, Some(tree_position), reach, ORDER)?;
                return Ok(());
            }
            if now + 1e-9 < worker.next_hit {
                return Ok(());
            }
            *work = work
                .checked_sub(1)
                .ok_or_else(|| self.harvest_error("harvest work exhausted"))?;
            let damage = self
                .slow_chopping_for(worker.unit)?
                .map_or(worker.rules.damage, |(damage, _)| damage);
            let plan = self.prepare_destructable_damage(tree, damage)?;
            let capacity = worker.rules.lumber_capacity as f32;
            let Some(plan) = plan else {
                worker.next_hit += f64::from(work_interval(&worker.rules));
                continue;
            };
            let life = plan.life();
            worker.carrying = (worker.carrying + worker.rules.damage).min(capacity);
            worker.next_hit += f64::from(work_interval(&worker.rules));
            let mut route = Ok(());
            let mut reseek = false;
            if life <= 0.0 {
                worker.tree = None;
                if worker.carrying >= 1.0 {
                    worker.phase = Phase::ToDepot;
                    worker.depot = None;
                    worker.reroute = true;
                    worker.phase_since = now;
                    route = self.route_depot(worker, position);
                } else {
                    worker.phase = Phase::Seek;
                    worker.depot = None;
                    reseek = true;
                }
            } else if worker.carrying + 1e-3 >= capacity {
                worker.phase = Phase::ToDepot;
                worker.depot = None;
                worker.reroute = true;
                worker.phase_since = now;
                route = self.route_depot(worker, position);
            }
            self.commit_destructable_damage(plan);
            if worker.phase == Phase::Work {
                self.enter_work_animation(worker.unit, now)?;
            }
            route?;
            if reseek {
                self.seek_tree(worker, position)?;
            }
            if worker.phase != Phase::Work {
                return Ok(());
            }
        }
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
        let owner = self.executed_unit(worker.unit)?.owner;
        worker.owner = owner;
        let usable = self
            .executed_unit(depot)
            .is_ok_and(|record| record.shown && !record.combat.as_ref().is_some_and(|c| c.dead))
            && self.depot_accepts(depot, owner, worker.gold.is_some())?;
        if !usable {
            worker.depot = None;
            return self.route_depot(worker, position);
        }
        let depot_position = self.unit_position(depot)?;
        let reach = self.depot_reach(depot, &worker.rules)?;
        if distance(position, depot_position) > reach {
            if worker.reroute || !self.activity_moving(worker.unit)? {
                if !self.steer_unit_in_range(worker.unit, Some(depot_position), reach, ORDER)? {
                    return Err(self.harvest_error("worker cannot move to the selected depot"));
                }
                if self.activity_waypoints_empty(worker.unit)? {
                    return Err(self.harvest_error("selected depot is not reachable"));
                }
                worker.reroute = false;
                worker.phase_since = now;
            }
            return Ok(());
        }
        let amount = if let Some(gold) = &worker.gold {
            gold.carrying
        } else if worker.carrying >= 1.0 {
            self.slow_chopping_for(worker.unit)?
                .map_or(worker.carrying.floor() as i32, |(_, deposit)| deposit)
        } else {
            0
        };
        if amount > 0 {
            let credit = if worker.gold.is_some() {
                self.prepare_gold_credit_reserving(owner, amount, 0)?
            } else {
                self.prepare_lumber_credit_reserving(owner, amount, 0)?
            };
            self.stop_unit(worker.unit)?;
            self.apply_resource_credit(credit);
        } else {
            self.stop_unit(worker.unit)?;
        }
        worker.carrying = 0.0;
        if let Some(gold) = &mut worker.gold {
            gold.carrying = 0;
        }
        if worker.manual_return {
            let state = self.harvest.as_mut().expect("admitted");
            state
                .workers
                .retain(|existing| existing.unit != worker.unit);
            state.cargo.remove(&worker.unit);
            return Ok(());
        }
        worker.phase = Phase::Seek;
        worker.tree = None;
        if worker.gold.is_some() {
            return self.seek_gold(worker, position);
        }
        self.seek_tree(worker, position)?;
        Ok(())
    }

    fn slow_chopping_for(&self, unit: HandleId) -> Result<Option<(f32, i32)>, Error> {
        let Some(policy) = self.campaign_slow_chopping_policy() else {
            return Ok(None);
        };
        let record = self.executed_unit(unit)?;
        let player = self.player_settings(record.owner)?.number;
        if !self.campaign_ai.is_active(player) {
            return Ok(None);
        }
        match self.campaign_ai.slow_chopping(player) {
            None => {
                return Err(self.harvest_error("active campaign AI slow-chopping flag is unset"));
            }
            Some(false) => return Ok(None),
            Some(true) => {}
        }
        // shortcut: only direct, effect-free workers have qualified slow-chop packets; admit modifiers when measured.
        if record.summoned || record.admitted_order.is_some() || self.unit_has_effect_history(unit)
        {
            return Err(self
                .harvest_error("slow-chopping worker has unqualified ability or effect metadata"));
        }
        Ok(Some((policy.tree_damage, policy.deposit_lumber)))
    }

    #[cfg(test)]
    pub(crate) fn test_set_harvest_admitted_order(&mut self, unit: HandleId, active: bool) {
        let Entry::Unit { record } = self.inner.entry_mut(unit).expect("unit fixture") else {
            panic!("unit fixture");
        };
        record.admitted_order = Some((*b"Aoff", active));
    }

    fn route_depot(&mut self, worker: &mut Worker, position: [f32; 2]) -> Result<(), Error> {
        let owner = self.executed_unit(worker.unit)?.owner;
        let Some(depot) = self.select_depot(
            worker.unit,
            owner,
            position,
            &worker.rules,
            worker.gold.is_some(),
        )?
        else {
            return Err(self.harvest_error("no resource-accepting depot within Ahar Area"));
        };
        let depot_position = self.unit_position(depot)?;
        let reach = self.depot_reach(depot, &worker.rules)?;
        worker.depot = Some(depot);
        worker.phase = Phase::ToDepot;
        worker.phase_since = self.timer_game_time().unwrap_or(0.0);
        worker.reroute = false;
        if distance(position, depot_position) > reach {
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
        gold: bool,
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
            if if gold {
                !depot.accepts_gold
            } else {
                !depot.accepts_lumber
            } {
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

    fn order_id(&self, name: &str) -> Option<i32> {
        self.order_registry.as_ref()?.id(name)
    }

    fn harvest_setup(
        &self,
        definition: &pcx_sim::world::Definition,
        owner: HandleId,
    ) -> Result<Option<HarvestSetup>, Error> {
        let (tables, abilities, upgrades) = {
            let state = self.harvest.as_ref().expect("admitted");
            (
                Arc::clone(&state.tables),
                state.abilities.clone(),
                state.upgrades.clone(),
            )
        };
        let Some(base_rules) = HarvestRules::from_definition(&tables, &abilities, definition)
            .map_err(|error| self.harvest_read_error("Ahar", error))?
        else {
            return Ok(None);
        };
        let upgrade_codes = definition_upgrade_codes(definition)
            .map_err(|error| self.harvest_read_error("upgradeList", error))?;
        let rules = self.apply_upgrades(&tables, &upgrades, definition, owner, &base_rules)?;
        Ok(Some(HarvestSetup {
            base_rules,
            rules,
            upgrade_codes,
        }))
    }

    fn apply_upgrades(
        &self,
        tables: &DefaultTables,
        upgrades: &Overlay,
        definition: &pcx_sim::world::Definition,
        owner: HandleId,
        base_rules: &HarvestRules,
    ) -> Result<HarvestRules, Error> {
        let owner_number = self
            .player_settings(owner)
            .ok()
            .map(|settings| settings.number);
        let rawcode = String::from_utf8_lossy(&definition.rawcode).into_owned();
        let mut rules = base_rules.clone();
        rules
            .apply_lumber_harvesting_upgrades(tables, upgrades, definition, |tech| {
                let Some(number) = owner_number else {
                    return Err(HarvestError::Missing {
                        rawcode: rawcode.clone(),
                        column: "player".into(),
                        detail: "player settings unavailable for applied research".into(),
                    });
                };
                match self.stored_tech_level(number, tech) {
                    super::players::TechRead::Level(level) => Ok(level),
                    super::players::TechRead::Absent => Ok(0),
                    super::players::TechRead::Unavailable => Err(HarvestError::Missing {
                        rawcode: rawcode.clone(),
                        column: "applied level".into(),
                        detail: "applied research state unavailable".into(),
                    }),
                }
            })
            .map_err(|error| self.harvest_read_error("upgradeList", error))?;
        Ok(rules)
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

    fn depot_accepts(
        &mut self,
        depot: HandleId,
        owner: HandleId,
        gold: bool,
    ) -> Result<bool, Error> {
        let Ok(record) = self.executed_unit(depot) else {
            return Ok(false);
        };
        if record.owner != owner {
            return Ok(false);
        }
        let rawcode = record.definition.rawcode;
        Ok(self.depot_return(rawcode)?.is_some_and(|depot| {
            if gold {
                depot.accepts_gold
            } else {
                depot.accepts_lumber
            }
        }))
    }

    fn stop_unit(&mut self, unit: HandleId) -> Result<(), Error> {
        self.steer_unit_in_range(unit, None, 0.0, ORDER)?;
        Ok(())
    }

    fn require_work_animation(&self, unit: HandleId) -> Result<(), Error> {
        let Entry::Unit { record } = self.inner.entry(unit)? else {
            return Err(self.harvest_error("unit handle required"));
        };
        let Some(animation) = &record.animation else {
            return Err(self.harvest_error("worker lacks an admitted animation model"));
        };
        if !animation.has_sequence("attack") {
            return Err(self.harvest_error("worker lacks the attack work sequence"));
        }
        Ok(())
    }

    fn enter_work_animation(&mut self, unit: HandleId, now: f64) -> Result<(), Error> {
        let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
            return Err(self.harvest_error("unit handle required"));
        };
        let Some(animation) = &mut record.animation else {
            return Err(self.harvest_error("worker lacks an admitted animation model"));
        };
        if !animation.has_sequence("attack") {
            return Err(self.harvest_error("worker lacks the attack work sequence"));
        }
        animation
            .select("attack", now, None)
            .map_err(|detail| self.harvest_error(detail))?;
        Ok(())
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
    let Some(weapon) = &rules.work_weapon else {
        return Err("worker has no admitted tree-targeting weapon");
    };
    if !weapon.range.is_finite() || weapon.range < 0.0 {
        return Err("tree weapon range must be finite and nonnegative");
    }
    if !weapon.damage_point.is_finite() || weapon.damage_point < 0.0 {
        return Err("tree weapon damage point must be finite and nonnegative");
    }
    if work_interval(rules) <= 0.0 || !work_interval(rules).is_finite() {
        return Err("harvest work interval must be finite and positive");
    }
    Ok(())
}

fn tree_reach(rules: &HarvestRules) -> f32 {
    rules.range
}

fn work_interval(rules: &HarvestRules) -> f32 {
    rules.work_weapon.as_ref().expect("validated").cool
}

fn work_delay(rules: &HarvestRules) -> f32 {
    rules.work_weapon.as_ref().expect("validated").damage_point
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
    fn reach_uses_the_ahar_range() {
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
        assert_eq!(tree_reach(&rules), 200.0);
        assert_eq!(work_interval(&rules), 2.0);
        assert_eq!(work_delay(&rules), 0.5);
    }
}

//! Deferred range entries over the same accepted unit records used by getters.
use super::{Entry, Error, Failure, ObjectHost, TriggerDelivery};
use pcx_script::{HandleId, Value, ast::Expr, value::FunctionId};
use std::sync::{Arc, Weak};

const DELIVERY: &str = "unit range delivery";

pub(super) struct State {
    owner: Arc<()>,
    generation: u64,
    event: HandleId,
    limit: usize,
    members: Vec<(HandleId, HandleId)>,
    active: Option<(HandleId, HandleId)>,
}

/// One non-cloneable entry, valid only on its originating arena and poll.
#[derive(Debug)]
pub struct UnitRangeEntry {
    owner: Weak<()>,
    generation: u64,
    registration: HandleId,
    unit: HandleId,
    filter: Option<FunctionId>,
}

impl ObjectHost {
    pub(super) fn range_context(&self) -> Option<(HandleId, HandleId)> {
        self.unit_range_runtime.as_ref().and_then(|s| s.active)
    }

    pub(super) fn set_range_context(&mut self, active: Option<(HandleId, HandleId)>) {
        if let Some(state) = &mut self.unit_range_runtime {
            state.active = active;
        }
    }

    pub(super) fn forget_hidden_unit_ranges(&mut self, unit: HandleId) {
        if let Some(state) = &mut self.unit_range_runtime {
            state.members.retain(|(_, candidate)| *candidate != unit);
        }
    }

    /// The event identity comes from the selected common source, not an ordinal.
    pub fn admit_unit_range_runtime(
        &mut self,
        global_name: &str,
        max_pairs: usize,
    ) -> Result<(), Error> {
        self.retained_unit_in_range()?;
        let fail = |why| self.inner.error(DELIVERY, why);
        if self.unit_range_runtime.is_some()
            || self.trigger_runtime.is_none()
            || self.units.is_none()
            || max_pairs == 0
        {
            return Err(fail(Failure::UnitState(
                "range registration, units and trigger runtime required, once",
            )));
        }
        std::alloc::Layout::array::<(HandleId, HandleId)>(max_pairs)
            .map_err(|_| fail(Failure::Capacity))?;
        let source = &self
            .inner
            .program
            .natives()
            .get("TriggerRegisterUnitInRange")
            .expect("range admission checked declaration")
            .source;
        for name in ["GetTriggerUnit", "GetFilterUnit", "GetEnteringUnit"] {
            if name == "GetEnteringUnit" && self.inner.program.natives().get(name).is_none() {
                continue;
            }
            Self::contract(&self.inner, name, &[], "unit", true, Some("unit"))?;
            if self
                .inner
                .program
                .natives()
                .get(name)
                .expect("contract checked")
                .source
                != *source
            {
                return Err(fail(Failure::Source));
            }
        }
        let global = self
            .inner
            .program
            .globals()
            .iter()
            .find(|g| g.name == global_name)
            .ok_or_else(|| fail(Failure::Initializer(global_name.into())))?;
        let invalid = || fail(Failure::Initializer(global_name.into()));
        if !global.is_constant
            || global.array
            || global.ty.array
            || global.ty.kind.name() != "unitevent"
            || global.source != *source
        {
            return Err(invalid());
        }
        let Some(Expr::Call { callee, args, .. }) = &global.init else {
            return Err(invalid());
        };
        if !matches!(callee.as_ref(), Expr::Ident(name, _) if name.as_str()=="ConvertUnitEvent")
            || args.len() != 1
        {
            return Err(invalid());
        }
        let mut work = 4096;
        let number = super::super::integer(&args[0], 64, &mut work).map_err(fail)?;
        let Value::Handle(event) = self
            .inner
            .convert_for("ConvertUnitEvent", &[Value::Int(number)])?
        else {
            unreachable!()
        };
        self.unit_range_runtime = Some(State {
            owner: Arc::new(()),
            generation: 0,
            event,
            limit: max_pairs,
            members: Vec::new(),
            active: None,
        });
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn range_test_position(&mut self, unit: HandleId, position: Option<[f32; 2]>) {
        let Entry::Unit { record } = self.inner.entry_mut(unit).expect("fixture unit") else {
            panic!("fixture unit family")
        };
        record.position = position;
    }

    pub fn unit_range_runtime_enabled(&self) -> bool {
        self.unit_range_runtime.is_some()
    }

    /// Sample final accepted positions once per explicit simulation boundary.
    /// Failure consumes work but leaves the previous membership and generation intact.
    pub fn poll_unit_ranges(&mut self, work: &mut usize) -> Result<Vec<UnitRangeEntry>, Error> {
        let fail = |why| self.inner.error(DELIVERY, why);
        let state = self
            .unit_range_runtime
            .as_ref()
            .ok_or_else(|| fail(Failure::UnitState("range runtime missing")))?;
        if state.active.is_some()
            || !self.callbacks.is_empty()
            || self.timer_delivery_active()
            || self
                .trigger_runtime
                .as_ref()
                .is_some_and(|s| s.active.is_some())
        {
            return Err(fail(Failure::UnitState(
                "poll requires a completed script boundary",
            )));
        }
        let generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| fail(Failure::Capacity))?;
        *work = work
            .checked_sub(self.inner.arena.len())
            .ok_or_else(|| fail(Failure::Budget("range arena scan")))?;
        let mut registrations = Vec::new();
        for (index, entry) in self.inner.arena.iter().enumerate() {
            if let Entry::RegisteredUnitInRange(record) = entry {
                registrations
                    .try_reserve(1)
                    .map_err(|_| fail(Failure::Capacity))?;
                registrations.push((index, *record));
            }
        }
        if registrations.is_empty() {
            return Ok(Vec::new());
        }
        let mut units = Vec::new();
        for (id, unit) in self.executed_units() {
            if !unit.shown {
                continue;
            }
            let position = unit
                .position
                .ok_or_else(|| fail(Failure::UnitState("accepted range position missing")))?;
            if position.iter().any(|v| !v.is_finite()) {
                return Err(fail(Failure::UnitState("nonfinite range position")));
            }
            units.try_reserve(1).map_err(|_| fail(Failure::Capacity))?;
            units.push((id, position, self.unit_collision_radius(id, DELIVERY)?));
        }
        let mut members = Vec::new();
        let mut entrants = Vec::new();
        // shortcut: registration/creation-order snapshot; qualify native ordering and mid-dispatch changes before extending it.
        for (index, record) in registrations {
            if record.range < 0.0 {
                return Err(fail(Failure::UnitState(
                    "negative range geometry is unqualified",
                )));
            }
            let event = self.inner.id(index, DELIVERY)?;
            if matches!(self.inner.entry(record.unit)?, Entry::RemovedUnit) {
                continue;
            }
            let anchor = self
                .executed_unit(record.unit)?
                .position
                .ok_or_else(|| fail(Failure::UnitState("accepted range anchor missing")))?;
            for &(unit, point, collision) in &units {
                *work = work
                    .checked_sub(1)
                    .ok_or_else(|| fail(Failure::Budget("range pair scan")))?;
                if unit == record.unit {
                    continue;
                }
                let radius = f64::from(record.range) + f64::from(collision);
                let dx = f64::from(point[0]) - f64::from(anchor[0]);
                let dy = f64::from(point[1]) - f64::from(anchor[1]);
                // shortcut: inclusive candidate-collision circle matches retained boundary cases; qualify equality, eligibility and native cadence before widening fidelity claims.
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
                if members.len() >= state.limit {
                    return Err(fail(Failure::Budget("range memberships")));
                }
                members
                    .try_reserve(1)
                    .map_err(|_| fail(Failure::Capacity))?;
                members.push((event, unit));
                if state.members.binary_search(&(event, unit)).is_ok() {
                    continue;
                }
                let filter = record
                    .filter
                    .map(|id| match self.inner.entry(id)? {
                        Entry::Filter { .. } => self.filter_callback(id).map(|r| r.function),
                        Entry::Condition { .. } => self.condition_callback(id).map(|r| r.function),
                        _ => Err(fail(Failure::WrongFamily)),
                    })
                    .transpose()?;
                entrants
                    .try_reserve(1)
                    .map_err(|_| fail(Failure::Capacity))?;
                entrants.push(UnitRangeEntry {
                    owner: Arc::downgrade(&state.owner),
                    generation,
                    registration: event,
                    unit,
                    filter,
                });
            }
        }
        let state = self.unit_range_runtime.as_mut().expect("checked runtime");
        state.members = members;
        state.generation = generation;
        Ok(entrants)
    }

    /// Establish filter/unit context only. The trigger/event context starts after acceptance.
    pub fn begin_unit_range_entry(
        &mut self,
        entry: UnitRangeEntry,
    ) -> Result<Option<FunctionId>, Error> {
        let fail = |why| self.inner.error(DELIVERY, why);
        let state = self
            .unit_range_runtime
            .as_ref()
            .ok_or_else(|| fail(Failure::Identity))?;
        if !Weak::ptr_eq(&entry.owner, &Arc::downgrade(&state.owner))
            || state.generation != entry.generation
            || state.active.is_some()
            || !self.callbacks.is_empty()
            || self.timer_delivery_active()
            || self
                .trigger_runtime
                .as_ref()
                .is_some_and(|s| s.active.is_some())
        {
            return Err(fail(Failure::UnitState(
                "stale, foreign or nested range delivery",
            )));
        }
        self.unit_in_range_registration(entry.registration)?;
        self.executed_unit(entry.unit)?;
        self.unit_range_runtime.as_mut().expect("checked").active =
            Some((entry.registration, entry.unit));
        Ok(entry.filter)
    }

    /// Recheck trigger lifetime/enabled state after the filter's script effects.
    pub fn begin_unit_range_trigger(&mut self) -> Result<Option<TriggerDelivery>, Error> {
        if self
            .trigger_runtime
            .as_ref()
            .is_some_and(|state| state.active.is_some())
        {
            return Err(self
                .inner
                .error(DELIVERY, Failure::UnitState("unfinished range trigger")));
        }
        let state = self
            .unit_range_runtime
            .as_ref()
            .ok_or_else(|| self.inner.error(DELIVERY, Failure::Identity))?;
        let (event, _) = state
            .active
            .ok_or_else(|| self.inner.error(DELIVERY, Failure::Identity))?;
        let record = self.unit_in_range_registration(event)?;
        if matches!(self.inner.entry(record.trigger)?, Entry::DeadTrigger)
            || self.trigger_enabled(record.trigger)? == Some(false)
        {
            return Ok(None);
        }
        let delivery = self.trigger_delivery(record.trigger, DELIVERY)?;
        let state = self
            .trigger_runtime
            .as_mut()
            .expect("range admission requires runtime");
        state.active = Some(record.trigger);
        state.event_context = Some(self.unit_range_runtime.as_ref().expect("checked").event);
        Ok(Some(delivery))
    }

    pub fn finish_unit_range_entry(&mut self) {
        if let Some(state) = &mut self.unit_range_runtime
            && state.active.take().is_some()
        {
            self.finish_timer_trigger();
        }
    }

    pub(super) fn range_filter_unit(&self) -> Option<HandleId> {
        self.unit_range_runtime
            .as_ref()?
            .active
            .map(|(_, unit)| unit)
    }

    pub(super) fn unit_range_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.unit_range_runtime.as_ref()?;
        if name != "GetTriggerUnit"
            && !(name == "GetEnteringUnit" && self.inner.program.natives().get(name).is_some())
        {
            return None;
        }
        Some(if args.is_empty() {
            Ok(
                if name == "GetEnteringUnit"
                    && self
                        .trigger_runtime
                        .as_ref()
                        .is_none_or(|state| state.event_context.is_none())
                {
                    Value::Null
                } else {
                    self.range_filter_unit().map_or(Value::Null, Value::Handle)
                },
            )
        } else {
            Err(self
                .inner
                .error(name, Failure::UnitState("no arguments expected")))
        })
    }
}

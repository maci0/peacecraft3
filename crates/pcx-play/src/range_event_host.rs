//! Source-bound `TriggerRegisterUnitInRange` records.
//!
//! Registration returns an event and does not move the unit, query terrain,
//! or run the filter. Range delivery still needs an accepted live position.
use super::{Entry, Error, Failure, ObjectHost, loaded_type};
use pcx_script::{HandleId, Value};

pub(super) const REGISTER: &str = "TriggerRegisterUnitInRange";

/// One retained registration. `range` is the executed real, not a collision formula.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitInRangeRegistration {
    pub trigger: HandleId,
    pub unit: HandleId,
    pub range: f32,
    pub filter: Option<HandleId>,
}

impl ObjectHost {
    /// Admit the loaded declaration. Omission leaves the native unsupported.
    ///
    /// The native must return `event` and share `CreateTrigger`'s source.
    /// `unit` stays under `widget`. No position, collision or polling value is supplied.
    pub fn with_unit_in_range(mut self, max_records: usize) -> Result<Self, Error> {
        if self.admitted_triggers()?.unit_in_range_limit.is_some() {
            return Err(self.inner.error(
                REGISTER,
                Failure::TriggerState("unit-in-range events already admitted"),
            ));
        }
        std::alloc::Layout::array::<UnitInRangeRegistration>(max_records)
            .map_err(|_| self.inner.error(REGISTER, Failure::Capacity))?;
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "unit", "real", "boolexpr"],
            "event",
            false,
            Some("event"),
        )?;
        let source = &self
            .inner
            .program
            .natives()
            .get(REGISTER)
            .expect("checked contract")
            .source;
        for name in ["CreateTrigger", "CreateUnit"] {
            if let Some(native) = self.inner.program.natives().get(name)
                && &native.source != source
            {
                return Err(self.inner.error(REGISTER, Failure::Source));
            }
        }
        loaded_type(&self.inner, REGISTER, "unit", "widget", source)?;
        loaded_type(&self.inner, REGISTER, "event", "agent", source)?;
        if self.inner.program.types().any(|ty| ty.name == "boolexpr") {
            loaded_type(&self.inner, REGISTER, "boolexpr", "agent", source)?;
        }
        self.triggers
            .as_mut()
            .expect("admitted above")
            .unit_in_range_limit = Some(max_records);
        Ok(self)
    }

    pub fn retained_unit_in_range(&self) -> Result<usize, Error> {
        let admitted = self.admitted_triggers()?;
        if admitted.unit_in_range_limit.is_none() {
            return Err(self.inner.error(
                REGISTER,
                Failure::TriggerState("unit-in-range events not admitted"),
            ));
        }
        Ok(admitted.retained_unit_in_range)
    }

    pub fn unit_in_range_registration(
        &self,
        id: HandleId,
    ) -> Result<UnitInRangeRegistration, Error> {
        self.retained_unit_in_range()?;
        match self
            .inner
            .entry(id)
            .map_err(|error| self.inner.error(REGISTER, error.failure))?
        {
            Entry::RegisteredUnitInRange(record) => Ok(*record),
            _ => Err(self.inner.error(REGISTER, Failure::WrongFamily)),
        }
    }

    pub(super) fn register_unit_in_range(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |failure| self.inner.error(REGISTER, failure);
        let [Value::Handle(trigger), Value::Handle(unit), range, filter] = args else {
            return Err(fail(Failure::TriggerState(
                "expected trigger, unit, finite range and nullable filter",
            )));
        };
        let range = match range {
            Value::Real(value) if value.is_finite() => *value,
            _ => {
                return Err(fail(Failure::TriggerState("range must be a finite real")));
            }
        };
        if !matches!(
            self.inner
                .entry(*trigger)
                .map_err(|error| fail(error.failure))?,
            Entry::Trigger { .. }
        ) || !matches!(
            self.inner
                .entry(*unit)
                .map_err(|error| fail(error.failure))?,
            Entry::Unit { .. }
        ) {
            return Err(fail(Failure::WrongFamily));
        }
        let filter = match filter {
            Value::Null => None,
            Value::Handle(id) => {
                self.checked_boolean_wrapper(*id, REGISTER)?;
                Some(*id)
            }
            _ => {
                return Err(fail(Failure::TriggerState(
                    "expected nullable filter identity",
                )));
            }
        };
        let admitted = self.admitted_triggers()?;
        let limit = admitted
            .unit_in_range_limit
            .ok_or_else(|| fail(Failure::TriggerState("unit-in-range events not admitted")))?;
        if admitted.retained_unit_in_range >= limit {
            return Err(fail(Failure::Budget("unit-in-range registration")));
        }
        let id = self.inner.allocate(
            Entry::RegisteredUnitInRange(UnitInRangeRegistration {
                trigger: *trigger,
                unit: *unit,
                range,
                filter,
            }),
            REGISTER,
        )?;
        self.triggers
            .as_mut()
            .expect("admitted above")
            .retained_unit_in_range += 1;
        Ok(Value::Handle(id))
    }
}

//! Retained trigger/predicate associations; registration does not evaluate code.
use super::{Entry, Error, Failure, ObjectHost, loaded_type};
use pcx_script::{HandleId, Value};

pub(super) const ADD: &str = "TriggerAddCondition";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriggerConditionRegistration {
    pub trigger: HandleId,
    pub condition: HandleId,
}

impl ObjectHost {
    /// Admit associations separately from wrappers, trigger state and execution.
    pub fn with_trigger_conditions(mut self, max_records: usize) -> Result<Self, Error> {
        self.admitted_triggers()?;
        let fail = |failure| self.inner.error(ADD, failure);
        if self
            .triggers
            .as_ref()
            .is_some_and(|a| a.condition_limit.is_some())
        {
            return Err(fail(Failure::TriggerState(
                "trigger conditions already admitted",
            )));
        }
        std::alloc::Layout::array::<TriggerConditionRegistration>(max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        Self::contract(
            &self.inner,
            ADD,
            &["trigger", "boolexpr"],
            "triggercondition",
            false,
            Some("triggercondition"),
        )?;
        let source = &self
            .inner
            .program
            .natives()
            .get(ADD)
            .expect("checked contract")
            .source;
        if self
            .inner
            .program
            .natives()
            .get("CreateTrigger")
            .is_none_or(|n| &n.source != source)
        {
            return Err(fail(Failure::Source));
        }
        for (name, parent) in [("triggercondition", "agent"), ("boolexpr", "agent")] {
            loaded_type(&self.inner, ADD, name, parent, source)?;
        }
        self.triggers
            .as_mut()
            .expect("admitted above")
            .condition_limit = Some(max_records);
        Ok(self)
    }

    pub fn retained_trigger_conditions(&self) -> Result<usize, Error> {
        let admitted = self.admitted_triggers()?;
        if admitted.condition_limit.is_none() {
            return Err(self.inner.error(
                ADD,
                Failure::TriggerState("trigger conditions not admitted"),
            ));
        }
        Ok(admitted.retained_conditions)
    }

    pub fn trigger_condition_registration(
        &self,
        id: HandleId,
    ) -> Result<TriggerConditionRegistration, Error> {
        self.retained_trigger_conditions()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(ADD, e.failure))?
        {
            Entry::TriggerCondition(record) => Ok(*record),
            _ => Err(self.inner.error(ADD, Failure::WrongFamily)),
        }
    }

    pub(super) fn add_trigger_condition(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |failure| self.inner.error(ADD, failure);
        let [Value::Handle(trigger), Value::Handle(condition)] = args else {
            return Err(fail(Failure::TriggerState(
                "expected trigger and retained boolean wrapper",
            )));
        };
        if !matches!(
            self.inner.entry(*trigger).map_err(|e| fail(e.failure))?,
            Entry::Trigger
        ) {
            return Err(fail(Failure::WrongFamily));
        }
        self.checked_boolean_wrapper(*condition, ADD)?;
        let admitted = self.admitted_triggers()?;
        let limit = admitted
            .condition_limit
            .ok_or_else(|| fail(Failure::TriggerState("trigger conditions not admitted")))?;
        if admitted.retained_conditions >= limit {
            return Err(fail(Failure::Budget("trigger condition registration")));
        }
        let id = self.inner.allocate(
            Entry::TriggerCondition(TriggerConditionRegistration {
                trigger: *trigger,
                condition: *condition,
            }),
            ADD,
        )?;
        self.triggers
            .as_mut()
            .expect("admitted above")
            .retained_conditions += 1;
        Ok(Value::Handle(id))
    }
}

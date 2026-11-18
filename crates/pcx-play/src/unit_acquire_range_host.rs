//! Source default acquisition range and the qualified ordinary-unit current getter.
use super::combat::{column, number};
use super::{Error, Failure, ObjectHost};
use pcx_objects::editor::SlkId;
use pcx_script::Value;

impl ObjectHost {
    pub(super) fn acquire_range(&self, name: &str, args: &[Value]) -> Result<Value, Error> {
        Self::contract(&self.inner, name, &["unit"], "real", false, Some("unit"))?;
        let natives = self.inner.program.natives();
        if natives.get(name).map(|n| &n.source) != natives.get("CreateUnit").map(|n| &n.source) {
            return Err(self.inner.error(name, Failure::Source));
        }
        let fail = |reason: String| self.inner.error(name, Failure::UnitActivity(reason));
        let [Value::Handle(unit)] = args else {
            return Err(self.inner.error(name, Failure::Signature));
        };
        let record = self
            .executed_unit(*unit)
            .map_err(|error| self.inner.error(name, error.failure))?;
        let current = name == "GetUnitAcquireRange";
        if current && record.acquire_range.is_some() {
            return Err(fail(
                "current range after a raw acquire override is unsupported".into(),
            ));
        }
        let cell = column(&record.definition, SlkId::UnitWeapons, "acquire").map_err(fail)?;
        let range = cell
            .map(|value| number(value, "acquire"))
            .transpose()
            .map_err(fail)?
            .flatten()
            .ok_or_else(|| fail("the unit type has no acquire range cell".into()))?;
        // The native's current component has a distinct storage path from the type default.
        if current
            && (record.definition_revision != 0
                || record.cached
                || record.summoned
                || record.morph.is_some()
                || record.illusion.is_some()
                || record.construction.is_some()
                || !record.combat.as_ref().is_some_and(|combat| {
                    !combat.dead
                        && combat
                            .weapon
                            .as_ref()
                            .is_some_and(|weapon| weapon.acquire == range)
                }))
        {
            return Err(fail("current range is unqualified for this unit".into()));
        }
        Ok(Value::Real(range))
    }
}

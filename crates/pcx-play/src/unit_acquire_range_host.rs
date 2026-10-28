//! `GetUnitDefaultAcquireRange`: the unit type's acquisition range.
//!
//! The value is the unit's resolved `UnitWeapons` `acquire` column (the table the combat step
//! also reads for target acquisition), so map object edits apply. A unit whose `acquire` cell
//! is empty has no established default and refuses. `SetUnitAcquireRange` stores its value on
//! the unit but the combat step does not consume it yet, so no current-range getter exists.
use super::combat::{column, number};
use super::{Error, Failure, ObjectHost};
use pcx_objects::editor::SlkId;
use pcx_script::Value;

const NAME: &str = "GetUnitDefaultAcquireRange";

impl ObjectHost {
    pub(super) fn default_acquire_range(&self, args: &[Value]) -> Result<Value, Error> {
        Self::contract(&self.inner, NAME, &["unit"], "real", false, Some("unit"))?;
        let natives = self.inner.program.natives();
        if natives.get(NAME).map(|n| &n.source) != natives.get("CreateUnit").map(|n| &n.source) {
            return Err(self.inner.error(NAME, Failure::Source));
        }
        let fail = |reason: String| self.inner.error(NAME, Failure::UnitActivity(reason));
        let [Value::Handle(unit)] = args else {
            return Err(self.inner.error(NAME, Failure::Signature));
        };
        let record = self.executed_unit(*unit)?;
        let cell = column(&record.definition, SlkId::UnitWeapons, "acquire").map_err(fail)?;
        let range = cell
            .map(|value| number(value, "acquire"))
            .transpose()
            .map_err(fail)?
            .flatten()
            .ok_or_else(|| fail("the unit type has no acquire range cell".into()))?;
        Ok(Value::Real(range))
    }
}

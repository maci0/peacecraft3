//! `GetUnitUserData` and `SetUnitUserData`: one script-owned integer per unit, stored at
//! `[unit+0x488]` by the original (`docs/re-notes/unit-user-data.md`).
//!
//! A unit nothing has written reads 0: the unit constructor stores 0 there.

use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::Value;

const GET: &str = "GetUnitUserData";
const SET: &str = "SetUnitUserData";

impl ObjectHost {
    pub(super) fn unit_user_data_declarations(&self, source: &str) -> Result<(), Error> {
        for (name, args, result) in [
            (GET, &["unit"][..], "integer"),
            (SET, &["unit", "integer"][..], "nothing"),
        ] {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, args, result, false, Some("unit"))?;
            if native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        Ok(())
    }

    /// A null unit reads 0 and ignores a write (`0x483940` yields no unit); so does a removed one,
    /// whose handle no longer resolves.
    pub(super) fn unit_user_data_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !matches!(name, GET | SET) || self.inner.program.natives().get(name).is_none() {
            return None;
        }
        Some((|| {
            let (unit, write) = match (name, args) {
                (GET, [unit]) => (unit, None),
                (SET, [unit, Value::Int(data)]) => (unit, Some(*data)),
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            let unit = match unit {
                Value::Null => return Ok(missing_unit(write)),
                Value::Handle(unit) => *unit,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            if matches!(self.inner.entry(unit)?, Entry::RemovedUnit { .. }) {
                return Ok(missing_unit(write));
            }
            match write {
                Some(data) => {
                    let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
                        return Err(self.inner.error(name, Failure::WrongFamily));
                    };
                    record.user_data = data;
                    Ok(Value::Null)
                }
                None => Ok(Value::Int(self.executed_unit(unit)?.user_data)),
            }
        })())
    }
}

fn missing_unit(write: Option<i32>) -> Value {
    if write.is_some() {
        Value::Null
    } else {
        Value::Int(0)
    }
}

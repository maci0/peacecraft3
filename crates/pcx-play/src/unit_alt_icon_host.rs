use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::Value;

const NATIVE: &str = "UnitSetUsesAltIcon";

impl ObjectHost {
    pub(super) fn unit_alt_icon_declarations(&self, source: &str) -> Result<(), Error> {
        let Some(native) = self.inner.program.natives().get(NATIVE) else {
            return Ok(());
        };
        Self::contract(
            &self.inner,
            NATIVE,
            &["unit", "boolean"],
            "nothing",
            false,
            Some("unit"),
        )?;
        if native.source != source {
            return Err(self.inner.error(NATIVE, Failure::Source));
        }
        Ok(())
    }

    /// `UnitSetUsesAltIcon` (`0x4af800`) stores the flag in the unit's bit `0x200`
    /// of its state word at `+0x48c`. A null or removed unit is a no-op. No
    /// presentation reader of that bit is decoded yet, so the flag is state only.
    pub(super) fn unit_alt_icon_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != NATIVE || self.inner.program.natives().get(name).is_none() {
            return None;
        }
        Some((|| {
            let [unit, Value::Bool(flag)] = args else {
                return Err(self.inner.error(name, Failure::Signature));
            };
            let unit = match unit {
                Value::Null => return Ok(Value::Null),
                Value::Handle(unit) => *unit,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            match self.inner.entry_mut(unit)? {
                Entry::Unit { record } => record.uses_alt_icon = *flag,
                Entry::RemovedUnit { .. } => {}
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            }
            Ok(Value::Null)
        })())
    }
}

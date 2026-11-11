use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::Value;

const NATIVE: &str = "SetUnitCreepGuard";

impl ObjectHost {
    pub(super) fn unit_creep_guard_declaration(&self, source: &str) -> Result<(), Error> {
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

    /// `SetUnitCreepGuard` (`0x4ab2a0`) sets or clears bit `0x4` of the dword at unit
    /// `+0x168`. A null or removed unit is a no-op. Ability code tests that bit
    /// (`0xb18e78`, `0xb200f6`, `0xb2227b`, `0xb2392d`, `0xb24fb3`, `0xb250bd`); none of
    /// them is modeled, so the flag is state only.
    pub(super) fn unit_creep_guard_native(
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
                Entry::Unit { record } => record.creep_guard = *flag,
                Entry::RemovedUnit { .. } => {}
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            }
            Ok(Value::Null)
        })())
    }
}

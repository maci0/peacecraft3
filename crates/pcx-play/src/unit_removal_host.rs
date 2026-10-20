//! `RemoveUnit`: the handle becomes a tombstone in the same arena and every
//! runtime view of that unit (occupancy, motion, groups, ranges, effects) drops it.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::Value;

const REMOVE: &str = "RemoveUnit";

impl ObjectHost {
    pub(super) fn unit_removal_declaration(&self, source: &str) -> Result<(), Error> {
        let Some(native) = self.inner.program.natives().get(REMOVE) else {
            return Ok(());
        };
        Self::contract(
            &self.inner,
            REMOVE,
            &["unit"],
            "nothing",
            false,
            Some("unit"),
        )?;
        if native.source != source {
            return Err(self.inner.error(REMOVE, Failure::Source));
        }
        Ok(())
    }

    pub(super) fn unit_removal_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != REMOVE || self.units.is_none() {
            return None;
        }
        self.inner.program.natives().get(REMOVE)?;
        Some((|| {
            let unit = match args {
                [Value::Null] => return Ok(Value::Null),
                [Value::Handle(unit)] => *unit,
                _ => return Err(self.inner.error(REMOVE, Failure::Signature)),
            };
            match self
                .inner
                .entry(unit)
                .map_err(|e| self.inner.error(REMOVE, e.failure))?
            {
                Entry::RemovedUnit => return Ok(Value::Null),
                Entry::Unit { record } if record.shown => {
                    if let Some(placement) = &mut self.unit_placement {
                        placement.hide(unit).map_err(|e| {
                            self.inner
                                .error(REMOVE, Failure::UnitPlacement(e.to_string()))
                        })?;
                    }
                }
                Entry::Unit { .. } => {}
                _ => return Err(self.inner.error(REMOVE, Failure::WrongFamily)),
            }
            self.forget_hidden_unit_ranges(unit);
            self.forget_unit_activity(unit);
            self.members.retain(|&(_, member)| member != unit);
            for entry in &mut self.inner.arena {
                if let Entry::Effect {
                    unit: target, live, ..
                } = entry
                    && *target == unit
                {
                    *live = false;
                }
            }
            *self.inner.entry_mut(unit)? = Entry::RemovedUnit;
            Ok(Value::Null)
        })())
    }
}

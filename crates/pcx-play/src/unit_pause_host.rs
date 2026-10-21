use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::Value;

impl ObjectHost {
    pub(super) fn unit_pause_declarations(&self, source: &str) -> Result<(), Error> {
        for (name, args, result) in [
            ("PauseUnit", &["unit", "boolean"][..], "nothing"),
            ("IsUnitPaused", &["unit"][..], "boolean"),
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

    pub(super) fn unit_pause_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !matches!(name, "PauseUnit" | "IsUnitPaused")
            || self.inner.program.natives().get(name).is_none()
        {
            return None;
        }
        Some((|| {
            let (unit, pause) = match (name, args) {
                ("PauseUnit", [unit, Value::Bool(pause)]) => (unit, Some(*pause)),
                ("IsUnitPaused", [unit]) => (unit, None),
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            let unit = match unit {
                Value::Null => {
                    return Ok(if pause.is_some() {
                        Value::Null
                    } else {
                        Value::Bool(false)
                    });
                }
                Value::Handle(unit) => *unit,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            if matches!(self.inner.entry(unit)?, Entry::RemovedUnit) {
                return Ok(if pause.is_some() {
                    Value::Null
                } else {
                    Value::Bool(false)
                });
            }
            let record = self.executed_unit(unit)?;
            let since = record.paused_since;
            let Some(pause) = pause else {
                return Ok(Value::Bool(since.is_some()));
            };
            if pause == since.is_some() {
                return Ok(Value::Null);
            }
            let now = self.timer_game_time().ok_or_else(|| {
                self.inner
                    .error(name, Failure::UnitState("pause requires game clock"))
            })?;
            self.check_activity_boundary(now, name)?;
            if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                if let Some(since) = since {
                    if let Some(activity) = &mut record.activity {
                        activity.resume_after_pause(since, now);
                    }
                    if let Some(combat) = &mut record.combat {
                        combat.resume_after_pause(now - since);
                    }
                    if let Some(vitals) = &mut record.vitals {
                        vitals.regen.through = now;
                    }
                }
                record.paused_since = pause.then_some(now);
            }
            if let Some(since) = since {
                self.resume_summon_cast(unit, now - since);
                self.guard_resumed(unit, since, now);
            }
            Ok(Value::Null)
        })())
    }
}

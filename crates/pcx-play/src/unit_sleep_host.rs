//! Natural sleep permission and state on the executed unit; the day window comes from the clock.
use super::{Entry, Error, ExecutedUnit, Failure, ObjectHost};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::{HandleId, Value};
use pcx_sim::world::{Definition, FieldValue};

const NATIVES: [(&str, &[&str], &str); 4] = [
    ("UnitAddSleep", &["unit", "boolean"], "nothing"),
    ("UnitCanSleep", &["unit"], "boolean"),
    ("UnitIsSleeping", &["unit"], "boolean"),
    ("UnitWakeUp", &["unit"], "nothing"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitSleep {
    permitted: bool,
    sleeping: bool,
    wake_pending: bool,
}

fn permission(field: &FieldValue) -> Result<bool, &'static str> {
    match field {
        FieldValue::Cell(value) if value == "0" => Ok(false),
        FieldValue::Cell(value) if value == "1" => Ok(true),
        FieldValue::Override(pcx_objects::Value::Int(0)) => Ok(false),
        FieldValue::Override(pcx_objects::Value::Int(1)) => Ok(true),
        _ => Err("canSleep must be a source boolean"),
    }
}

impl ExecutedUnit {
    pub fn is_sleeping(&self) -> bool {
        self.sleep.is_some_and(|s| s.sleeping)
    }
}

impl ObjectHost {
    pub(super) fn unit_sleep_declarations(&self, source: &str) -> Result<(), Error> {
        for (name, args, result) in NATIVES {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, args, result, false, Some("unit"))?;
                if native.source != source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        Ok(())
    }

    pub(super) fn prepare_unit_sleep(
        &self,
        definition: &Definition,
    ) -> Result<Option<UnitSleep>, Error> {
        if !NATIVES
            .iter()
            .any(|(name, ..)| self.inner.program.natives().get(name).is_some())
        {
            return Ok(None);
        }
        let fail = |detail| self.inner.error("CreateUnit", Failure::UnitState(detail));
        let field = super::unit_positions::field(
            definition,
            SlkId::UnitData,
            "canSleep",
            &[FieldKind::Bool],
        )
        .map_err(fail)?;
        let permitted = permission(field).map_err(fail)?;
        Ok(Some(UnitSleep {
            permitted,
            sleeping: false,
            wake_pending: false,
        }))
    }

    pub(super) fn unit_sleep_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !NATIVES.iter().any(|(n, ..)| *n == name)
            || self.inner.program.natives().get(name).is_none()
        {
            return None;
        }
        Some((|| {
            let (unit, permission) = match (name, args) {
                ("UnitAddSleep", [unit, Value::Bool(add)]) => (unit, Some(*add)),
                ("UnitCanSleep" | "UnitIsSleeping" | "UnitWakeUp", [unit]) => (unit, None),
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            let query = matches!(name, "UnitCanSleep" | "UnitIsSleeping");
            let empty = if query {
                Value::Bool(false)
            } else {
                Value::Null
            };
            let id = match unit {
                Value::Null => return Ok(empty),
                Value::Handle(id) => *id,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            if matches!(self.inner.entry(id)?, Entry::RemovedUnit) {
                return Ok(empty);
            }
            let state = self.executed_unit(id)?.sleep.ok_or_else(|| {
                self.inner
                    .error(name, Failure::UnitState("sleep source not admitted"))
            })?;
            if query {
                return Ok(Value::Bool(if name == "UnitCanSleep" {
                    state.permitted
                } else {
                    state.sleeping
                }));
            }
            if name == "UnitWakeUp" {
                self.wake_natural_sleep(id);
            } else if let Entry::Unit { record } = self.inner.entry_mut(id)? {
                let sleep = record.sleep.as_mut().expect("checked");
                sleep.permitted = permission.expect("setter");
                if !sleep.permitted {
                    sleep.sleeping = false;
                    sleep.wake_pending = false;
                }
            }
            Ok(Value::Null)
        })())
    }

    pub(super) fn wake_natural_sleep(&mut self, unit: HandleId) {
        if let Ok(Entry::Unit { record }) = self.inner.entry_mut(unit)
            && let Some(sleep) = &mut record.sleep
        {
            sleep.sleeping = false;
            sleep.wake_pending = sleep.permitted;
        }
    }

    pub(super) fn advance_unit_sleep(&mut self, work: &mut usize) -> Result<(), Error> {
        let units: Vec<_> = self
            .executed_units()
            .filter(|(_, u)| u.sleep.is_some())
            .map(|(id, _)| id)
            .collect();
        for unit in units {
            *work = work.checked_sub(1).ok_or_else(|| {
                self.inner
                    .error("unit sleep", Failure::Budget("unit tick work"))
            })?;
            let record = self.executed_unit(unit)?;
            if record.paused_since.is_some() {
                continue;
            }
            let state = record.sleep.expect("filtered");
            if !state.permitted {
                continue;
            }
            let night = self.natural_sleep_night().ok_or_else(|| {
                self.inner.error(
                    "unit sleep",
                    Failure::ClockState("sleep needs the source day/night window"),
                )
            })?;
            let dead = record.combat.as_ref().is_some_and(|c| c.dead)
                || record.vitals.is_some_and(|v| v.life <= 0.0);
            let busy = record
                .activity
                .as_ref()
                .is_some_and(|a| a.moving() || a.patrolling() || a.following())
                || record
                    .combat
                    .as_ref()
                    .is_some_and(|c| c.target.is_some() || c.attack_destination.is_some());
            if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                let sleep = record.sleep.as_mut().expect("filtered");
                // shortcut: allow one activity pass after waking; native resleep timing/proximity remains unqualified.
                sleep.sleeping = night && !dead && !busy && !state.wake_pending;
                sleep.wake_pending = false;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn natural_sleep_permission_preserves_bool_cells_and_overrides_and_refuses_other_values() {
        for (cell, expected) in [("0", false), ("1", true)] {
            assert_eq!(permission(&FieldValue::Cell(cell.into())), Ok(expected));
            assert_eq!(
                permission(&FieldValue::Override(pcx_objects::Value::Int(u32::from(
                    expected
                )))),
                Ok(expected)
            );
        }
        for value in [
            FieldValue::Cell("2".into()),
            FieldValue::Cell("-".into()),
            FieldValue::Cell("".into()),
            FieldValue::Override(pcx_objects::Value::Int(u32::MAX)),
            FieldValue::Override(pcx_objects::Value::Real(1.0)),
        ] {
            assert!(permission(&value).is_err());
        }
    }
}

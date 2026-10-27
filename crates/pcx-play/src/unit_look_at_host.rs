use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

/// Script-selected BONE/HELP joint and a live target; offsets use source world axes.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitLookAt {
    pub bone: String,
    pub target: HandleId,
    pub offset: [f32; 3],
}

impl ObjectHost {
    pub(super) fn unit_look_at_declarations(&self, source: &str) -> Result<(), Error> {
        for (name, args) in [
            (
                "SetUnitLookAt",
                &["unit", "string", "unit", "real", "real", "real"][..],
            ),
            ("ResetUnitLookAt", &["unit"][..]),
        ] {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, args, "nothing", false, Some("unit"))?;
            if native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        Ok(())
    }

    pub(super) fn unit_look_at_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !matches!(name, "SetUnitLookAt" | "ResetUnitLookAt")
            || self.inner.program.natives().get(name).is_none()
        {
            return None;
        }
        Some((|| {
            let (unit, request) = match (name, args) {
                ("ResetUnitLookAt", [unit]) => (unit, None),
                (
                    "SetUnitLookAt",
                    [
                        unit,
                        Value::Str(bone),
                        target,
                        Value::Real(x),
                        Value::Real(y),
                        Value::Real(z),
                    ],
                ) => {
                    if ![x, y, z].iter().all(|v| v.is_finite()) {
                        return Err(self
                            .inner
                            .error(name, Failure::UnitState("nonfinite look-at offset")));
                    }
                    (unit, Some((bone, target, [*x, *y, *z])))
                }
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            let unit = match unit {
                Value::Null => return Ok(Value::Null),
                Value::Handle(id) => *id,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            if matches!(self.inner.entry(unit)?, Entry::RemovedUnit { .. }) {
                return Ok(Value::Null);
            }
            let record = self.executed_unit(unit)?;
            let state = if let Some((bone, target, offset)) = request {
                let target = match target {
                    Value::Null => return Ok(Value::Null),
                    Value::Handle(id) => *id,
                    _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                };
                if matches!(self.inner.entry(target)?, Entry::RemovedUnit { .. }) {
                    return Ok(Value::Null);
                }
                self.executed_unit(target)?;
                let model = &record
                    .animation
                    .as_ref()
                    .ok_or_else(|| {
                        self.inner.error(
                            name,
                            Failure::UnitState("look-at requires an admitted unit model"),
                        )
                    })?
                    .model;
                if model
                    .joints
                    .iter()
                    .filter(|b| b.name.eq_ignore_ascii_case(bone))
                    .count()
                    != 1
                {
                    return Err(self.inner.error(
                        name,
                        Failure::UnitAnimation(format!(
                            "{}: look-at bone {bone:?} is missing or ambiguous",
                            model.member
                        )),
                    ));
                }
                Some(UnitLookAt {
                    bone: bone.clone(),
                    target,
                    offset,
                })
            } else {
                None
            };
            if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                record.look_at = state;
            }
            Ok(Value::Null)
        })())
    }
}

use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::Value;

pub(super) struct PreparedVisibility {
    pub position: Option<[f32; 2]>,
    shown: bool,
    changed: bool,
    placement: Option<crate::unit_placement::Prepared>,
    relocated: bool,
    restart: Option<f64>,
}

impl ObjectHost {
    pub(super) fn unit_visibility_declarations(&self, source: &str) -> Result<(), Error> {
        for (name, args, result, constant) in [
            ("ShowUnit", &["unit", "boolean"][..], "nothing", false),
            ("IsUnitHidden", &["unit"][..], "boolean", true),
        ] {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, args, result, constant, Some("unit"))?;
            if native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        Ok(())
    }

    pub(super) fn unit_visibility_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !matches!(name, "ShowUnit" | "IsUnitHidden")
            || self.inner.program.natives().get(name).is_none()
        {
            return None;
        }
        Some((|| {
            let (unit, shown) = match (name, args) {
                ("ShowUnit", [unit, Value::Bool(shown)]) => (unit, Some(*shown)),
                ("IsUnitHidden", [unit]) => (unit, None),
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            let unit = match unit {
                Value::Null => {
                    return Ok(if shown.is_some() {
                        Value::Null
                    } else {
                        Value::Bool(false)
                    });
                }
                Value::Handle(unit) => *unit,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            let record = self
                .executed_unit(unit)
                .map_err(|e| self.inner.error(name, e.failure))?;
            let Some(shown) = shown else {
                return Ok(Value::Bool(!record.shown));
            };
            let prepared = self.prepare_unit_visibility(unit, shown, name)?;
            self.commit_unit_visibility(unit, prepared, name)?;
            Ok(Value::Null)
        })())
    }

    pub(super) fn prepare_unit_visibility(
        &mut self,
        unit: pcx_script::HandleId,
        shown: bool,
        name: &str,
    ) -> Result<PreparedVisibility, Error> {
        let record = self.executed_unit(unit)?;
        if record.shown == shown {
            return Ok(PreparedVisibility {
                position: record.position,
                shown,
                changed: false,
                placement: None,
                relocated: false,
                restart: None,
            });
        }
        let geometry = if shown
            && self.unit_placement.is_some()
            && record.combat.as_ref().is_none_or(|combat| !combat.dead)
        {
            let geometry = super::unit_positions::geometry(&record.definition)
                .map_err(|s| self.inner.error(name, Failure::UnitState(s)))?;
            Some((geometry.radius, geometry.path.to_owned(), geometry.movement))
        } else {
            None
        };
        let (position, face, moving) = (
            record.position,
            record.face,
            record.activity.as_ref().is_some_and(|a| a.moving()),
        );
        let mut relocated = None;
        let prepared = if let Some((radius, path, movement)) = geometry {
            let position = position.ok_or_else(|| {
                self.inner
                    .error(name, Failure::UnitState("accepted reveal position missing"))
            })?;
            let prepared = self
                .unit_placement
                .as_mut()
                .expect("placement")
                .prepare_show_class(unit, position, radius, &path, face, movement)
                .map_err(|e| {
                    self.inner
                        .error(name, Failure::UnitPlacement(e.to_string()))
                })?;
            if prepared.position != position {
                relocated = Some(prepared.position);
            }
            Some(prepared)
        } else {
            None
        };
        let restart = if moving && relocated.is_some() {
            Some(self.timer_game_time().ok_or_else(|| {
                self.inner
                    .error(name, Failure::UnitState("moving unit clock missing"))
            })?)
        } else {
            None
        };
        if relocated.is_some() {
            self.require_blight_projection(unit, name)?;
        }
        Ok(PreparedVisibility {
            position: relocated.or(position),
            shown,
            changed: true,
            placement: prepared,
            relocated: relocated.is_some(),
            restart,
        })
    }

    pub(super) fn commit_unit_visibility(
        &mut self,
        unit: pcx_script::HandleId,
        prepared: PreparedVisibility,
        name: &str,
    ) -> Result<(), Error> {
        if !prepared.changed {
            return Ok(());
        }
        let shown = prepared.shown;
        if let Some(placement) = prepared.placement {
            self.unit_placement
                .as_mut()
                .expect("placement")
                .commit(unit, placement);
        } else if !shown && let Some(placement) = &mut self.unit_placement {
            placement.hide(unit).map_err(|e| {
                self.inner
                    .error(name, Failure::UnitPlacement(e.to_string()))
            })?;
        }
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            record.shown = shown;
            if prepared.relocated
                && let Some(position) = prepared.position
            {
                record.position = Some(position);
                if let (Some(activity), Some(now)) = (&mut record.activity, prepared.restart) {
                    activity.relocated(position, now);
                }
            }
        }
        if !shown {
            self.forget_hidden_unit_ranges(unit);
        }
        Ok(())
    }
}

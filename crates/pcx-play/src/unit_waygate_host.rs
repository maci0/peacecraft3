//! Waygate natives. The `Awrp` ability on a unit owns a destination and an
//! active flag (`docs/re-notes/waygate.md`). A unit without `Awrp` in its
//! `UnitAbilities/abilList` takes the original's missing-ability path: setters
//! do nothing and getters return `0.0` or `false`.

use super::hero::ability_list;
use super::{Entry, Error, Failure, ObjectHost};
use pcx_objects::editor::{FieldSource, SlkId};
use pcx_script::{HandleId, Value};

/// `Awrp`, the ability code of the waygate ability (`Units\AbilityData.slk`).
const WAYGATE_ABILITY: [u8; 4] = *b"Awrp";

/// Waygate state written by `WaygateSetDestination` and `WaygateActivate`.
///
/// `None` means the native has not written that field on the unit. Reads of an
/// unwritten field refuse because the original's default for it is not decoded.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Waygate {
    pub destination: Option<[f32; 2]>,
    pub active: Option<bool>,
}

impl ObjectHost {
    pub(super) fn unit_waygate_declarations(&self, source: &str) -> Result<(), Error> {
        for (name, args, result) in [
            ("WaygateGetDestinationX", &["unit"][..], "real"),
            ("WaygateGetDestinationY", &["unit"][..], "real"),
            (
                "WaygateSetDestination",
                &["unit", "real", "real"][..],
                "nothing",
            ),
            ("WaygateActivate", &["unit", "boolean"][..], "nothing"),
            ("WaygateIsActive", &["unit"][..], "boolean"),
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

    /// Whether the unit owns `Awrp`. A definition without an `abilList` column has no
    /// UnitAbilities row and owns no abilities. The list is the bounded cell, so no
    /// budget applies.
    fn owns_waygate_ability(&self, name: &str, unit: HandleId) -> Result<bool, Error> {
        let record = self.executed_unit(unit)?;
        let has_list = record.definition.fields.values().any(|field| {
            field.metadata.as_ref().is_some_and(|m| {
                matches!(&m.source, FieldSource::Column { slk: SlkId::UnitAbilities, column }
                    if column.as_str() == "abilList")
            })
        });
        if !has_list {
            return Ok(false);
        }
        Ok(ability_list(&record.definition, "abilList", usize::MAX)
            .map_err(|e| self.inner.error(name, Failure::UnitActivity(e)))?
            .contains(&WAYGATE_ABILITY))
    }

    /// Whether the unit is a waygate whose `WaygateActivate` flag is set. The
    /// move-order transfer that reads this flag is not decoded, so callers refuse.
    pub(crate) fn waygate_active(&self, name: &str, unit: HandleId) -> Result<bool, Error> {
        if matches!(self.inner.entry(unit)?, Entry::RemovedUnit { .. })
            || !self.owns_waygate_ability(name, unit)?
        {
            return Ok(false);
        }
        Ok(self.executed_unit(unit)?.waygate.active == Some(true))
    }

    pub(super) fn unit_waygate_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !matches!(
            name,
            "WaygateGetDestinationX"
                | "WaygateGetDestinationY"
                | "WaygateSetDestination"
                | "WaygateActivate"
                | "WaygateIsActive"
        ) || self.inner.program.natives().get(name).is_none()
        {
            return None;
        }
        Some((|| {
            let (unit, write) = match (name, args) {
                (
                    "WaygateGetDestinationX" | "WaygateGetDestinationY" | "WaygateIsActive",
                    [unit],
                ) => (unit, None),
                ("WaygateSetDestination", [unit, Value::Real(x), Value::Real(y)]) => {
                    if !x.is_finite() || !y.is_finite() {
                        return Err(self
                            .inner
                            .error(name, Failure::UnitState("finite destination required")));
                    }
                    (unit, Some(Write::Destination([*x, *y])))
                }
                ("WaygateActivate", [unit, Value::Bool(active)]) => {
                    (unit, Some(Write::Active(*active)))
                }
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            let unit = match unit {
                Value::Null => return Ok(default_result(name, write.is_some())),
                Value::Handle(unit) => *unit,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            if matches!(self.inner.entry(unit)?, Entry::RemovedUnit { .. })
                || !self.owns_waygate_ability(name, unit)?
            {
                return Ok(default_result(name, write.is_some()));
            }
            let waygate = self.executed_unit(unit)?.waygate;
            match (name, write) {
                ("WaygateSetDestination" | "WaygateActivate", Some(write)) => {
                    let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
                        return Err(self.inner.error(name, Failure::WrongFamily));
                    };
                    match write {
                        Write::Destination(point) => record.waygate.destination = Some(point),
                        Write::Active(active) => record.waygate.active = Some(active),
                    }
                    Ok(Value::Null)
                }
                ("WaygateGetDestinationX", None) => {
                    Ok(Value::Real(self.read_destination(name, &waygate)?[0]))
                }
                ("WaygateGetDestinationY", None) => {
                    Ok(Value::Real(self.read_destination(name, &waygate)?[1]))
                }
                ("WaygateIsActive", None) => Ok(Value::Bool(waygate.active.ok_or_else(|| {
                    self.inner.error(
                        name,
                        Failure::UnitState("waygate active flag was never written"),
                    )
                })?)),
                _ => Err(self.inner.error(name, Failure::Signature)),
            }
        })())
    }

    fn read_destination(&self, name: &str, waygate: &Waygate) -> Result<[f32; 2], Error> {
        waygate.destination.ok_or_else(|| {
            self.inner.error(
                name,
                Failure::UnitState("waygate destination was never written"),
            )
        })
    }
}

enum Write {
    Destination([f32; 2]),
    Active(bool),
}

/// Result for a null, removed or non-waygate unit: setters are no-ops and
/// getters return the zero that the original's missing-ability path returns.
fn default_result(name: &str, setter: bool) -> Value {
    if setter {
        return Value::Null;
    }
    match name {
        "WaygateIsActive" => Value::Bool(false),
        _ => Value::Real(0.),
    }
}

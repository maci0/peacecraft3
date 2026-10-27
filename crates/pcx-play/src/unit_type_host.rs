//! `IsUnitType` from each unit's resolved definition and live state. Type
//! names resolve through the script's own `UNIT_TYPE_*` constants. Status
//! types without a modeled effect (stunned, ...) read false. Natural sleep uses live unit state.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::Value;
use pcx_sim::world::{Definition, FieldValue};

const NATIVE: &str = "IsUnitType";

pub(super) fn structure(definition: &Definition) -> Result<bool, String> {
    let field =
        super::unit_positions::field(definition, SlkId::UnitBalance, "isbldg", &[FieldKind::Bool])
            .map_err(|e| format!("UnitBalance/isbldg: {e}"))?;
    match field {
        FieldValue::Cell(value) if value.trim() == "0" => Ok(false),
        FieldValue::Cell(value) if value.trim() == "1" => Ok(true),
        FieldValue::Override(pcx_objects::Value::Int(0)) => Ok(false),
        FieldValue::Override(pcx_objects::Value::Int(1)) => Ok(true),
        _ => Err("UnitBalance/isbldg requires a source boolean".into()),
    }
}

/// Script type names whose suffix is the `UnitBalance/type` source token.
const CLASSES: [&str; 8] = [
    "UNIT_TYPE_GIANT",
    "UNIT_TYPE_UNDEAD",
    "UNIT_TYPE_MECHANICAL",
    "UNIT_TYPE_PEON",
    "UNIT_TYPE_SAPPER",
    "UNIT_TYPE_TOWNHALL",
    "UNIT_TYPE_ANCIENT",
    "UNIT_TYPE_TAUREN",
];

pub(super) fn class(definition: &Definition, global: &str) -> Result<bool, String> {
    let token = global
        .strip_prefix("UNIT_TYPE_")
        .ok_or("unit class requires a source type name")?;
    let list = super::unit_positions::text(definition, SlkId::UnitBalance, "type")
        .map_err(|e| format!("UnitBalance/type: {e}"))?;
    Ok(list
        .split(',')
        .any(|part| part.trim().eq_ignore_ascii_case(token)))
}

impl ObjectHost {
    pub(super) fn unit_type_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != NATIVE || self.units.is_none() {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some((|| {
            let [unit, Value::Handle(kind)] = args else {
                return Err(self.inner.error(name, Failure::Signature));
            };
            let Entry::Symbolic {
                kind: family,
                enumerand,
            } = self.inner.entry(*kind)?
            else {
                return Err(self.inner.error(name, Failure::WrongFamily));
            };
            if family != "unittype" {
                return Err(self.inner.error(name, Failure::WrongFamily));
            }
            let record = match unit {
                Value::Null => return Ok(Value::Bool(false)),
                Value::Handle(id) => match self.inner.entry(*id)? {
                    Entry::Unit { record } => record,
                    Entry::RemovedUnit { .. } => return Ok(Value::Bool(false)),
                    _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                },
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            let is = |global: &str| {
                self.source_constant_enumerand(global, "unittype")
                    .is_ok_and(|v| v == *enumerand)
            };
            let definition = &record.definition;
            let weapon = record.combat.as_ref().and_then(|c| c.weapon);
            let value = if is("UNIT_TYPE_HERO") {
                record.illusion.is_none() && definition.rawcode[0].is_ascii_uppercase()
            } else if is("UNIT_TYPE_DEAD") {
                record.combat.as_ref().is_some_and(|c| c.dead)
                    || record.vitals.is_some_and(|v| v.life <= 0.0)
            } else if is("UNIT_TYPE_SLEEPING") {
                record.is_sleeping()
            } else if is("UNIT_TYPE_SUMMONED") {
                record.summoned
            } else if is("UNIT_TYPE_STRUCTURE") {
                structure(definition)
                    .map_err(|e| self.inner.error(NATIVE, Failure::UnitActivity(e)))?
            } else if is("UNIT_TYPE_FLYING") {
                record
                    .movement()
                    .map_err(|e| self.inner.error(NATIVE, Failure::UnitActivity(e.into())))?
                    == crate::unit_placement::Movement::Air
            } else if is("UNIT_TYPE_GROUND") {
                record
                    .movement()
                    .map_err(|e| self.inner.error(NATIVE, Failure::UnitActivity(e.into())))?
                    != crate::unit_placement::Movement::Air
            } else if is("UNIT_TYPE_MELEE_ATTACKER") {
                weapon.is_some_and(|w| w.missile_speed.is_none())
            } else if is("UNIT_TYPE_RANGED_ATTACKER") {
                weapon.is_some_and(|w| w.missile_speed.is_some())
            } else if let Some(&global) = CLASSES.iter().find(|global| is(global)) {
                class(definition, global)
                    .map_err(|e| self.inner.error(NATIVE, Failure::UnitActivity(e)))?
            } else {
                false
            };
            Ok(Value::Bool(value))
        })())
    }
}

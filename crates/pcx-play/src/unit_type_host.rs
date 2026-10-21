//! `IsUnitType` from each unit's resolved definition and live state. Type
//! names resolve through the script's own `UNIT_TYPE_*` constants. Status
//! types without a modeled effect (stunned, ...) read false. Natural sleep uses live unit state.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_objects::editor::SlkId;
use pcx_script::Value;

const NATIVE: &str = "IsUnitType";

/// Classification tokens of `UnitBalance` `type` named by these unit types.
const CLASSES: [(&str, &str); 8] = [
    ("UNIT_TYPE_GIANT", "giant"),
    ("UNIT_TYPE_UNDEAD", "undead"),
    ("UNIT_TYPE_MECHANICAL", "mechanical"),
    ("UNIT_TYPE_PEON", "peon"),
    ("UNIT_TYPE_SAPPER", "sapper"),
    ("UNIT_TYPE_TOWNHALL", "townhall"),
    ("UNIT_TYPE_ANCIENT", "ancient"),
    ("UNIT_TYPE_TAUREN", "tauren"),
];

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
                    Entry::RemovedUnit => return Ok(Value::Bool(false)),
                    _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                },
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            let is = |global: &str| {
                self.source_constant_enumerand(global, "unittype")
                    .is_ok_and(|v| v == *enumerand)
            };
            let definition = &record.definition;
            let text = |slk, column| super::unit_positions::text(definition, slk, column);
            let movement = text(SlkId::UnitData, "movetp").unwrap_or("");
            let structure =
                super::unit_activity::balance_real(definition, "isbldg").is_ok_and(|v| v != 0.0);
            let weapon = record.combat.as_ref().and_then(|c| c.weapon);
            let value = if is("UNIT_TYPE_HERO") {
                definition.rawcode[0].is_ascii_uppercase()
            } else if is("UNIT_TYPE_DEAD") {
                record.combat.as_ref().is_some_and(|c| c.dead)
                    || record.vitals.is_some_and(|v| v.life <= 0.0)
            } else if is("UNIT_TYPE_SLEEPING") {
                record.is_sleeping()
            } else if is("UNIT_TYPE_STRUCTURE") {
                structure
            } else if is("UNIT_TYPE_FLYING") {
                movement == "fly"
            } else if is("UNIT_TYPE_GROUND") {
                movement != "fly"
            } else if is("UNIT_TYPE_MELEE_ATTACKER") {
                weapon.is_some_and(|w| w.missile_speed.is_none())
            } else if is("UNIT_TYPE_RANGED_ATTACKER") {
                weapon.is_some_and(|w| w.missile_speed.is_some())
            } else if let Some(&(_, token)) = CLASSES.iter().find(|(global, _)| is(global)) {
                text(SlkId::UnitBalance, "type").is_ok_and(|list| {
                    list.split(',')
                        .any(|t| t.trim().eq_ignore_ascii_case(token))
                })
            } else {
                false
            };
            Ok(Value::Bool(value))
        })())
    }
}

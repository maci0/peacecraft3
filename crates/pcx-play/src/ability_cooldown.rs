//! Script remaining/end queries share the implemented casts' existing deadlines.
use super::*;

pub(super) const NATIVES: [(&str, &str); 2] = [
    ("BlzGetUnitAbilityCooldownRemaining", "real"),
    ("BlzEndUnitAbilityCooldown", "nothing"),
];

impl ObjectHost {
    pub(super) fn ability_cooldown_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        let fail = |detail: &str| self.hero_error(name, detail.into());
        let end = name == NATIVES[1].0;
        let absent = if end { Value::Null } else { Value::Real(0.0) };
        let [unit, Value::Int(ability)] = args else {
            return Err(fail("unit and ability rawcode required"));
        };
        let unit = match unit {
            Value::Null => return Ok(absent),
            Value::Handle(unit) => *unit,
            _ => return Err(fail("unit identity required")),
        };
        if matches!(self.inner.entry(unit)?, Entry::RemovedUnit) {
            return Ok(absent);
        }
        let ability = ability.to_be_bytes();
        let record = self.created_unit(unit, name)?;
        if record
            .abilities
            .as_ref()
            .is_none_or(|abilities| abilities.level(ability) <= 0)
        {
            return Ok(absent);
        }
        // Visibility, death and player availability do not replace the wrapper's unit/ability gate.
        let admitted = match &ability {
            b"AHhb" => self.holy_light.is_some(),
            b"AHwe" => self.summoning.is_some(),
            _ => false,
        };
        if !admitted {
            return Err(fail(&format!(
                "cooldown behavior for present ability {ability:?} is not admitted"
            )));
        }
        let deadline = match &ability {
            b"AHhb" => {
                let ready = &mut self.holy_light.as_mut().expect("checked").ready_at;
                if end {
                    ready.remove(&unit)
                } else {
                    ready.get(&unit).copied()
                }
            }
            b"AHwe" => {
                let ready = &mut self.summoning.as_mut().expect("checked").ready;
                if end {
                    ready.remove(&unit)
                } else {
                    ready.get(&unit).copied()
                }
            }
            _ => unreachable!("checked implemented ability"),
        };
        if end {
            return Ok(Value::Null);
        }
        let Some(deadline) = deadline else {
            return Ok(absent);
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.inner.error(name, Failure::UnitState("clock missing")))?;
        Ok(Value::Real((deadline - now).max(0.0) as f32))
    }
}

//! Script cooldown controls share the implemented casts' configuration and deadlines.
use super::*;

pub(super) const NATIVES: [(&str, &[&str], &str); 4] = [
    (
        "BlzGetUnitAbilityCooldownRemaining",
        &["unit", "integer"],
        "real",
    ),
    ("BlzEndUnitAbilityCooldown", &["unit", "integer"], "nothing"),
    (
        "BlzGetUnitAbilityCooldown",
        &["unit", "integer", "integer"],
        "real",
    ),
    (
        "BlzSetUnitAbilityCooldown",
        &["unit", "integer", "integer", "real"],
        "nothing",
    ),
];

impl ObjectHost {
    pub(crate) fn unit_cooldown_reset_declaration(&self, source: &str) -> Result<(), Error> {
        let name = "UnitResetCooldown";
        if let Some(native) = self.inner.program.natives().get(name) {
            Self::contract(&self.inner, name, &["unit"], "nothing", false, Some("unit"))?;
            if native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        Ok(())
    }

    pub(crate) fn clear_unit_cooldowns(&mut self, unit: HandleId) {
        if let Some(holy) = &mut self.holy_light {
            holy.ready_at.remove(&unit);
        }
        if let Some(summoning) = &mut self.summoning {
            summoning.ready.remove(&unit);
        }
        self.reset_item_cooldowns(unit);
    }

    pub(crate) fn unit_reset_cooldown(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "UnitResetCooldown";
        let unit = match args {
            [Value::Null] => return Ok(Value::Null),
            [Value::Handle(unit)] => *unit,
            _ => return Err(self.inner.error(name, Failure::Signature)),
        };
        if matches!(self.inner.entry(unit)?, Entry::RemovedUnit) {
            return Ok(Value::Null);
        }
        self.created_unit(unit, name)?;
        self.clear_unit_cooldowns(unit);
        Ok(Value::Null)
    }

    pub(super) fn ability_cooldown_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        let fail = |detail: &str| self.hero_error(name, detail.into());
        if name == NATIVES[2].0 || name == NATIVES[3].0 {
            return self.ability_cooldown_duration_call(name, args);
        }
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

    pub(crate) fn ability_cooldown_duration(
        &self,
        unit: HandleId,
        ability: [u8; 4],
        rank: i32,
    ) -> Result<f32, Error> {
        let fail = |detail| self.hero_error("ability cooldown duration", detail);
        if let Some(bits) = self
            .created_unit(unit, "ability cooldown duration")?
            .abilities
            .as_ref()
            .and_then(|state| {
                state
                    .cooldowns
                    .iter()
                    .find(|(alias, level, _)| *alias == ability && *level == rank)
                    .map(|(_, _, bits)| *bits)
            })
        {
            return Ok(f32::from_bits(bits));
        }
        let raw = self
            .hero_ability_level_cell(ability, "Cool", rank, FieldKind::Unreal)
            .map_err(fail)?;
        let value = raw.parse::<f32>().map_err(|e| fail(e.to_string()))?;
        if !value.is_finite() || value < 0.0 {
            return Err(fail("cooldown must be finite and nonnegative".into()));
        }
        Ok(value)
    }

    fn ability_cooldown_duration_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        let fail = |detail: &str| self.hero_error(name, detail.into());
        let set = name == NATIVES[3].0;
        let absent = if set { Value::Null } else { Value::Real(0.0) };
        let (unit, ability, rank, value) = match args {
            [unit, Value::Int(ability), Value::Int(rank)] if !set => (unit, ability, rank, None),
            [
                unit,
                Value::Int(ability),
                Value::Int(rank),
                Value::Real(value),
            ] if set => (unit, ability, rank, Some(*value)),
            _ => {
                return Err(fail(
                    "unit, ability, one-based level and (for set) real duration required",
                ));
            }
        };
        if *rank < 1 {
            return Ok(absent);
        }
        let unit = match unit {
            Value::Null => return Ok(absent),
            Value::Handle(unit) => *unit,
            _ => return Err(fail("unit identity required")),
        };
        if matches!(self.inner.entry(unit)?, Entry::RemovedUnit) {
            return Ok(absent);
        }
        let ability = ability.to_be_bytes();
        if self
            .created_unit(unit, name)?
            .abilities
            .as_ref()
            .is_none_or(|state| state.level(ability) <= 0)
        {
            return Ok(absent);
        }
        if !match &ability {
            b"AHhb" => self.holy_light.is_some(),
            b"AHwe" => self.summoning.is_some(),
            _ => false,
        } {
            return Err(fail(
                "duration behavior for this present ability is not admitted",
            ));
        }
        let maximum = self
            .hero_ability_number(ability, "levels")
            .map_err(|detail| self.hero_error(name, detail))?;
        if *rank > maximum {
            return Ok(absent);
        }
        // Resolve the authored cell before retaining an override: source ranks bound storage.
        let default = self.ability_cooldown_duration(unit, ability, *rank)?;
        let Some(value) = value else {
            return Ok(Value::Real(default));
        };
        if !value.is_finite() || value < 0.0 {
            return Err(fail("negative/nonfinite duration behavior is not admitted"));
        }
        let error = fail("cooldown override allocation failed");
        let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
            unreachable!()
        };
        let overrides = &mut record.abilities.as_mut().expect("checked").cooldowns;
        if let Some((_, _, bits)) = overrides
            .iter_mut()
            .find(|(alias, level, _)| *alias == ability && *level == *rank)
        {
            *bits = value.to_bits();
        } else {
            overrides.try_reserve(1).map_err(|_| error)?;
            overrides.push((ability, *rank, value.to_bits()));
            overrides.sort_unstable_by_key(|(alias, rank, _)| (*alias, *rank));
        }
        Ok(Value::Null)
    }
}

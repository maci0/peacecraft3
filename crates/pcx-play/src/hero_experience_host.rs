use super::*;

pub(super) const NATIVES: [(&str, &[&str], &str); 5] = [
    ("GetHeroXP", &["unit"], "integer"),
    ("SetHeroXP", &["unit", "integer", "boolean"], "nothing"),
    ("AddHeroXP", &["unit", "integer", "boolean"], "nothing"),
    ("SuspendHeroXP", &["unit", "boolean"], "nothing"),
    ("IsSuspendedXP", &["unit"], "boolean"),
];
impl ObjectHost {
    pub(super) fn hero_experience_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        let unit = match (name, args) {
            ("GetHeroXP" | "IsSuspendedXP", [unit]) => unit,
            ("SuspendHeroXP", [unit, Value::Bool(_)]) => unit,
            ("SetHeroXP" | "AddHeroXP", [unit, Value::Int(_), Value::Bool(_)]) => unit,
            _ => return Err(self.hero_error(name, "invalid XP arguments".into())),
        };
        let null = || {
            if name == "GetHeroXP" {
                Value::Int(0)
            } else if name == "IsSuspendedXP" {
                Value::Bool(false)
            } else {
                Value::Null
            }
        };
        if matches!(unit, Value::Null) {
            return Ok(null());
        }
        let Value::Handle(unit) = unit else {
            return Err(self.hero_error(name, "XP unit identity required".into()));
        };
        let record = self.created_unit(*unit, name)?;
        let Some(hero) = record.abilities.as_ref().and_then(|a| a.hero.as_ref()) else {
            return Ok(null());
        };
        let state = hero.experience.ok_or_else(|| {
            self.hero_error(
                name,
                "XP is absent from this legacy cache or unadmitted".into(),
            )
        })?;
        match name {
            "GetHeroXP" => Ok(Value::Int(state.xp)),
            "IsSuspendedXP" => Ok(Value::Bool(state.suspended)),
            "SuspendHeroXP" => {
                let [_, Value::Bool(flag)] = args else {
                    unreachable!()
                };
                let Entry::Unit { record } = self.inner.entry_mut(*unit)? else {
                    unreachable!()
                };
                record
                    .abilities
                    .as_mut()
                    .expect("checked")
                    .hero
                    .as_mut()
                    .expect("checked")
                    .experience
                    .as_mut()
                    .expect("checked")
                    .suspended = *flag;
                Ok(Value::Null)
            }
            _ => {
                let [_, Value::Int(value), Value::Bool(eye)] = args else {
                    unreachable!()
                };
                if state.suspended {
                    return Ok(Value::Null);
                }
                if name == "AddHeroXP" && *value < 0 {
                    return Err(
                        self.hero_error(name, "negative XP addition is not qualified".into())
                    );
                }
                let xp = if name == "SetHeroXP" {
                    (*value).max(state.xp)
                } else {
                    state
                        .xp
                        .checked_add(*value)
                        .ok_or_else(|| self.hero_error(name, "XP overflow".into()))?
                };
                self.assign_hero_experience(*unit, xp, *eye)?;
                Ok(Value::Null)
            }
        }
    }
    pub(super) fn assign_hero_experience(
        &mut self,
        unit: HandleId,
        xp: i32,
        eye: bool,
    ) -> Result<(), Error> {
        let initial = self
            .hero_initial_level()
            .ok_or_else(|| self.hero_error("hero XP", "initial level absent".into()))?;
        let rules = self
            .admitted_units()?
            .heroes
            .as_ref()
            .and_then(|a| a.rules.experience.clone())
            .ok_or_else(|| self.hero_error("hero XP", "XP rules absent".into()))?;
        let next = rules
            .thresholds
            .level_for(initial, xp)
            .map_err(|e| self.hero_error("hero XP", e))?;
        self.change_hero_level(&[Value::Handle(unit), Value::Int(next), Value::Bool(eye)])?;
        let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
            unreachable!()
        };
        record
            .abilities
            .as_mut()
            .expect("admitted")
            .hero
            .as_mut()
            .expect("hero")
            .experience
            .as_mut()
            .expect("XP checked")
            .xp = xp;
        Ok(())
    }
}

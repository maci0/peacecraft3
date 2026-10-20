//! Live life and mana on executed units, from each unit's resolved
//! `UnitBalance` columns plus selected hero attribute bonuses. State names
//! resolve through the script's own `UNIT_STATE_*` constants.
use super::unit_activity::balance_real;
use super::{Entry, Error, Failure, ObjectHost};
use pcx_objects::editor::misc::HeroVitals;
use pcx_script::{HandleId, Value};
use pcx_sim::world::Definition;

const GET: &str = "GetUnitState";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vitals {
    pub life: f32,
    pub max_life: f32,
    pub mana: f32,
    pub max_mana: f32,
}

pub(super) struct Rules {
    bonuses: HeroVitals,
    /// Enumerands of LIFE, MAX_LIFE, MANA, MAX_MANA.
    states: [i32; 4],
}

/// An SLK `-` cell is empty; an empty mana column is no mana pool.
fn mana_column(definition: &Definition, column: &str) -> Result<f32, String> {
    match balance_real(definition, column) {
        Err(_) if definition.fields.values().any(|f| {
            f.metadata.as_ref().is_some_and(|m| {
                matches!(&m.source, pcx_objects::editor::FieldSource::Column { column: c, .. } if c == column)
            }) && matches!(&f.value, Ok(pcx_sim::world::FieldValue::Cell(s)) if s.trim() == "-")
        }) => Ok(0.0),
        other => other,
    }
}

impl ObjectHost {
    /// Before any unit exists; heroes (upper-case rawcode) add level-one attributes.
    pub fn admit_unit_vitals(&mut self, bonuses: HeroVitals) -> Result<(), Error> {
        if self.units.is_none()
            || self.executed_units().next().is_some()
            || self.unit_vitals.is_some()
        {
            return Err(self.inner.error(
                GET,
                Failure::UnitState("vitals require the unit catalog and no created units, once"),
            ));
        }
        Self::contract(
            &self.inner,
            GET,
            &["unit", "unitstate"],
            "real",
            true,
            Some("unit"),
        )?;
        let mut states = [0; 4];
        for (slot, global) in states.iter_mut().zip([
            "UNIT_STATE_LIFE",
            "UNIT_STATE_MAX_LIFE",
            "UNIT_STATE_MANA",
            "UNIT_STATE_MAX_MANA",
        ]) {
            *slot = self.source_constant_enumerand(global, "unitstate")?;
        }
        self.unit_vitals = Some(Rules { bonuses, states });
        Ok(())
    }

    /// Life per strength point from the admitted hero bonuses; `None` when vitals are not admitted.
    pub(super) fn life_per_strength(&self) -> Option<f32> {
        self.unit_vitals
            .as_ref()
            .map(|rules| rules.bonuses.life_per_strength)
    }

    pub(super) fn prepare_unit_vitals(
        &self,
        definition: &Definition,
    ) -> Result<Option<Vitals>, Error> {
        let Some(rules) = &self.unit_vitals else {
            return Ok(None);
        };
        let fail = |e: String| self.inner.error(GET, Failure::UnitActivity(e));
        let mut max_life = balance_real(definition, "HP").map_err(fail)?;
        let mut max_mana = mana_column(definition, "manaN").map_err(fail)?;
        let mut mana = mana_column(definition, "mana0").map_err(fail)?;
        if definition.rawcode[0].is_ascii_uppercase() {
            let strength = balance_real(definition, "STR").map_err(fail)?;
            let intelligence = balance_real(definition, "INT").map_err(fail)?;
            max_life += strength * rules.bonuses.life_per_strength;
            max_mana += intelligence * rules.bonuses.mana_per_intelligence;
            mana = max_mana;
        }
        Ok(Some(Vitals {
            life: max_life,
            max_life,
            mana: mana.min(max_mana),
            max_mana,
        }))
    }

    /// Stage the vital side of a level change before changing hero counters.
    /// Native level-up applies the change in attribute contribution to both pools.
    pub(super) fn prepare_hero_vital_level(
        &self,
        unit: HandleId,
        next: i32,
    ) -> Result<Option<Vitals>, Error> {
        let Some(rules) = &self.unit_vitals else {
            return Ok(None);
        };
        let record = self.executed_unit(unit)?;
        let Some(vitals) = record.vitals else {
            return Err(self
                .inner
                .error("SetHeroLevel", Failure::UnitState("vitals missing")));
        };
        let current = record
            .abilities
            .as_ref()
            .and_then(|a| a.hero.as_ref())
            .map(|h| h.level);
        let old = self.hero_initial_level().zip(current);
        let new = self.hero_initial_level().map(|initial| (initial, next));
        let bonus = self.unit_hero_bonus(unit)?;
        let definition = &record.definition;
        let result = || -> Result<Vitals, String> {
            let delta = |name: &str, coefficient: f32| -> Result<f32, String> {
                Ok((Self::derive_hero_attribute(definition, new, bonus, name)?
                    - Self::derive_hero_attribute(definition, old, bonus, name)?)
                    * coefficient)
            };
            let life = delta("STR", rules.bonuses.life_per_strength)?;
            let mana = delta("INT", rules.bonuses.mana_per_intelligence)?;
            adjust_hero_vitals(vitals, life, mana)
        };
        result()
            .map(Some)
            .map_err(|e| self.inner.error("SetHeroLevel", Failure::UnitActivity(e)))
    }

    pub fn unit_vitals(&self, unit: HandleId) -> Result<Option<Vitals>, Error> {
        Ok(self.executed_unit(unit)?.vitals)
    }

    /// `SetUnitState` on an admitted state: life and mana stay within their
    /// maximum. True when the assignment leaves a living unit without life.
    pub(super) fn assign_unit_vital(&mut self, unit: HandleId, enumerand: i32, value: f32) -> bool {
        let Some(states) = self.unit_vitals.as_ref().map(|r| r.states) else {
            return false;
        };
        let Ok(Entry::Unit { record }) = self.inner.entry_mut(unit) else {
            return false;
        };
        let Some(v) = &mut record.vitals else {
            return false;
        };
        match states.iter().position(|&s| s == enumerand) {
            Some(0) => v.life = value.min(v.max_life),
            Some(1) => {
                v.max_life = value;
                v.life = v.life.min(value);
            }
            Some(2) => v.mana = value.clamp(0.0, v.max_mana),
            Some(3) => {
                v.max_mana = value;
                v.mana = v.mana.min(value);
            }
            _ => {}
        }
        v.life <= 0.0 && record.combat.as_ref().is_some_and(|c| !c.dead)
    }

    pub(super) fn unit_vitals_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != GET {
            return None;
        }
        let rules = self.unit_vitals.as_ref()?;
        Some((|| {
            let [unit, Value::Handle(state)] = args else {
                return Err(self.inner.error(GET, Failure::Signature));
            };
            let Entry::Symbolic { kind, enumerand } = self.inner.entry(*state)? else {
                return Err(self.inner.error(GET, Failure::WrongFamily));
            };
            if kind != "unitstate" {
                return Err(self.inner.error(GET, Failure::WrongFamily));
            }
            let slot = rules
                .states
                .iter()
                .position(|s| s == enumerand)
                .ok_or_else(|| self.inner.error(GET, Failure::OutsideDomain(*enumerand)))?;
            // Null and removed units read as zero, so the BJ dead test holds.
            let vitals = match unit {
                Value::Null => return Ok(Value::Real(0.0)),
                Value::Handle(id) => match self.inner.entry(*id)? {
                    Entry::RemovedUnit => return Ok(Value::Real(0.0)),
                    Entry::Unit { record } => record.vitals.ok_or_else(|| {
                        self.inner.error(GET, Failure::UnitState("vitals missing"))
                    })?,
                    _ => return Err(self.inner.error(GET, Failure::WrongFamily)),
                },
                _ => return Err(self.inner.error(GET, Failure::WrongFamily)),
            };
            Ok(Value::Real(
                [vitals.life, vitals.max_life, vitals.mana, vitals.max_mana][slot],
            ))
        })())
    }
}

/// Preserve explicit maximum changes and the absolute deficit, rather than a percentage.
fn adjust_hero_vitals(old: Vitals, life_delta: f32, mana_delta: f32) -> Result<Vitals, String> {
    let mut next = Vitals {
        life: old.life + life_delta,
        max_life: old.max_life + life_delta,
        mana: old.mana + mana_delta,
        max_mana: old.max_mana + mana_delta,
    };
    if [
        old.life,
        old.max_life,
        old.mana,
        old.max_mana,
        life_delta,
        mana_delta,
        next.life,
        next.max_life,
        next.mana,
        next.max_mana,
    ]
    .iter()
    .any(|n| !n.is_finite())
        || next.max_life <= 0.0
        || next.max_mana < 0.0
    {
        return Err("invalid derived hero vital pools".into());
    }
    if old.life <= 0.0 || next.life <= 0.0 {
        return Err(
            "hero vital change across the living/dead minimum boundary is not derived".into(),
        );
    }
    next.life = next.life.min(next.max_life);
    next.mana = next.mana.clamp(0.0, next.max_mana);
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hero_vital_delta_preserves_absolute_deficit_and_clamps_mana() {
        let old = Vitals {
            life: 40.0,
            max_life: 100.0,
            mana: 7.0,
            max_mana: 40.0,
        };
        let next = adjust_hero_vitals(old, 25.0, 15.0).unwrap();
        assert_eq!(
            next,
            Vitals {
                life: 65.0,
                max_life: 125.0,
                mana: 22.0,
                max_mana: 55.0
            }
        );
        assert_ne!(next.life / next.max_life, old.life / old.max_life);
        let lower = adjust_hero_vitals(old, -15.0, -10.0).unwrap();
        assert_eq!(
            lower,
            Vitals {
                life: 25.0,
                max_life: 85.0,
                mana: 0.0,
                max_mana: 30.0
            }
        );
        for (life, mana) in [
            (-40.0, 0.0),
            (f32::INFINITY, 0.0),
            (0.0, f32::NAN),
            (0.0, -41.0),
        ] {
            assert!(adjust_hero_vitals(old, life, mana).is_err());
        }
    }
}

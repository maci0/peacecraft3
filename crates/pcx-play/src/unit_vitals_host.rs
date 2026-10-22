//! Live life and mana on executed units, from each unit's resolved
//! `UnitBalance` columns plus selected hero attribute bonuses. State names
//! resolve through the script's own `UNIT_STATE_*` constants.
//!
//! Regeneration integrates the per-second rates over game time between
//! advances. Day and night eligibility uses the admitted time-of-day clock.
use super::unit_activity::balance_real;
use super::{Entry, Error, Failure, ObjectHost};
use pcx_objects::editor::FieldSource;
use pcx_objects::editor::misc::HeroVitals;
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::{HandleId, Value};
use pcx_sim::world::{Definition, FieldValue};

const GET: &str = "GetUnitState";
const REGEN: &str = "unit regeneration";
type RegenMotion<'a> = (
    &'a [super::unit_activity::AcceptedSpan],
    &'a [(HandleId, [f32; 2])],
);

/// `UnitBalance` `regenType` tokens. Only the four stock tokens and `day` are admitted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RegenType {
    Always,
    Day,
    Night,
    Never,
    /// Blight terrain regeneration, requiring an admitted terrain layer.
    Blight,
}

/// Per-second rates before the hero attribute terms, and the life eligibility token.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Regen {
    pub life_per_second: f32,
    pub mana_per_second: f32,
    pub life_type: RegenType,
    /// Game time through which this unit's regeneration is integrated. Set to the
    /// creation time, so a unit never regenerates for time before it existed.
    pub through: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vitals {
    pub life: f32,
    pub max_life: f32,
    pub mana: f32,
    pub max_mana: f32,
    pub regen: Regen,
}

pub(super) struct Rules {
    bonuses: HeroVitals,
    /// Enumerands of LIFE, MAX_LIFE, MANA, MAX_MANA.
    states: [i32; 4],
}

/// An SLK `-` cell is empty and reads as zero for a numeric column.
fn balance_or_empty(definition: &Definition, column: &str) -> Result<f32, String> {
    match balance_real(definition, column) {
        Err(_)
            if definition.fields.values().any(|f| {
                f.metadata.as_ref().is_some_and(
                    |m| matches!(&m.source, FieldSource::Column { column: c, .. } if c == column),
                ) && matches!(&f.value, Ok(FieldValue::Cell(s)) if s.trim() == "-")
            }) =>
        {
            Ok(0.0)
        }
        other => other,
    }
}

/// The `UnitBalance` `regenType` token of a resolved unit.
fn regen_type(definition: &Definition) -> Result<RegenType, String> {
    let mut matches = definition.fields.iter().filter(|(id, f)| {
        f.metadata.as_ref().is_some_and(|m| {
            m.fourcc == **id
                && m.from == SlkId::UnitMetaData
                && matches!(m.index, -1 | 0)
                && m.kind == FieldKind::Token
                && matches!(&m.source, FieldSource::Column { slk: SlkId::UnitBalance, column } if column == "regenType")
        })
    });
    let (_, field) = matches.next().ok_or("missing unit balance regenType")?;
    if matches.next().is_some() {
        return Err("ambiguous unit balance regenType".into());
    }
    let FieldValue::Cell(token) = field
        .value
        .as_ref()
        .map_err(|e| format!("regenType: {e:?}"))?
    else {
        return Err("unsupported unit balance regenType field".into());
    };
    match token.trim() {
        "always" => Ok(RegenType::Always),
        "day" => Ok(RegenType::Day),
        "night" => Ok(RegenType::Night),
        "none" => Ok(RegenType::Never),
        "blight" => Ok(RegenType::Blight),
        other => Err(format!("unsupported regenType {other}")),
    }
}

impl ObjectHost {
    pub(super) fn require_blight_projection(
        &self,
        unit: HandleId,
        name: &str,
    ) -> Result<(), Error> {
        if self.initial_blight.is_some()
            && let Some(vitals) = self.executed_unit(unit)?.vitals
            && vitals.regen.life_type == RegenType::Blight
            && self
                .timer_game_time()
                .is_some_and(|now| now > vitals.regen.through)
        {
            return Err(self.inner.error(
                name,
                Failure::UnitActivity(
                    "blight relocation requires regeneration projected through the current clock"
                        .into(),
                ),
            ));
        }
        Ok(())
    }

    /// Admit map-authored blight once, before creations. Runtime setters remain
    /// unsupported until their player layers and painting rules are admitted.
    pub fn admit_initial_blight(
        &mut self,
        blight: crate::terrain_blight::InitialBlight,
    ) -> Result<(), Error> {
        if self.initial_blight.is_some()
            || self.executed_units().next().is_some()
            || self.unit_activity.is_none()
        {
            return Err(self.inner.error(
                REGEN,
                Failure::UnitActivity(
                    "initial blight requires activity and no created units, once".into(),
                ),
            ));
        }
        self.initial_blight = Some(blight);
        Ok(())
    }

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
        // Regeneration advances with the unit activity projection, so it needs that clock.
        if self.unit_activity.is_none() {
            return Err(self.inner.error(
                GET,
                Failure::UnitState("vitals regeneration requires admitted unit activity"),
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

    /// Mana per intelligence point from the admitted hero bonuses; `None` without vitals.
    pub(super) fn mana_per_intelligence(&self) -> Option<f32> {
        self.unit_vitals
            .as_ref()
            .map(|rules| rules.bonuses.mana_per_intelligence)
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
        let mut max_mana = balance_or_empty(definition, "manaN").map_err(fail)?;
        let mut mana = balance_or_empty(definition, "mana0").map_err(fail)?;
        let regen = Regen {
            life_per_second: balance_or_empty(definition, "regenHP").map_err(fail)?,
            mana_per_second: balance_or_empty(definition, "regenMana").map_err(fail)?,
            life_type: regen_type(definition).map_err(fail)?,
            through: self
                .timer_game_time()
                .ok_or_else(|| fail("regeneration requires the game clock".into()))?,
        };
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
            regen,
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

    /// Integrate each unit's life and mana regeneration from its own clock to `now`.
    /// Dead and nonpositive-life units do not regenerate. Errors leave every pool
    /// and clock unchanged, so a retry integrates the same spans.
    pub(super) fn advance_unit_regen(&mut self, now: f64) -> Result<(), Error> {
        let mut work = usize::MAX;
        self.advance_unit_regen_motion(now, None, &mut work)
    }

    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "simulation.regeneration")
    )]
    pub(super) fn advance_unit_regen_motion(
        &mut self,
        now: f64,
        motion: Option<RegenMotion<'_>>,
        work: &mut usize,
    ) -> Result<(), Error> {
        let Some(rules) = &self.unit_vitals else {
            return Ok(());
        };
        let bonuses = rules.bonuses;
        let fail = |e: String| self.inner.error(REGEN, Failure::UnitActivity(e));
        if !now.is_finite() {
            return Err(fail("finite game time required".into()));
        }
        let units: Vec<HandleId> = self
            .executed_units()
            .filter(|(_, u)| u.vitals.is_some())
            .map(|(id, _)| id)
            .collect();
        let mut next = Vec::with_capacity(units.len());
        for unit in units {
            let record = self.executed_unit(unit)?;
            let vitals = record.vitals.expect("filtered to vitals");
            let from = vitals.regen.through;
            if now < from {
                return Err(fail("unit regeneration clock moved backwards".into()));
            }
            if now == from {
                continue;
            }
            // The clock still moves on a dead or lifeless unit, so a later revive
            // cannot regenerate across the dead span.
            if record.paused_since.is_some()
                || record.combat.as_ref().is_some_and(|c| c.dead)
                || vitals.life <= 0.0
            {
                next.push((unit, vitals.life, vitals.mana));
                continue;
            }
            let elapsed = now - from;
            let (life_rate, mana_rate, bonus_life_rate) = self.regen_rates(unit, &bonuses)?;
            let daytime = self.daytime_between(from, now);
            let day = || match &daytime {
                Ok(Some(seconds)) => Ok(*seconds),
                Ok(None) => Err(fail(
                    "day/night regeneration needs the admitted time-of-day window".into(),
                )),
                Err(reason) => Err(fail(reason.clone())),
            };
            let life_seconds = if life_rate == 0.0 {
                0.0
            } else {
                match vitals.regen.life_type {
                    RegenType::Always => elapsed,
                    RegenType::Never => 0.0,
                    RegenType::Day => day()?,
                    RegenType::Night => elapsed - day()?,
                    RegenType::Blight => self.blight_seconds(unit, from, now, motion, work)?,
                }
            };
            let life =
                (f64::from(vitals.life) + life_rate * life_seconds + bonus_life_rate * elapsed)
                    .min(f64::from(vitals.max_life));
            let mana = (f64::from(vitals.mana) + mana_rate * elapsed)
                .clamp(0.0, f64::from(vitals.max_mana));
            next.push((unit, life as f32, mana as f32));
        }
        for (unit, life, mana) in next {
            if let Entry::Unit { record } = self.inner.entry_mut(unit)?
                && let Some(vitals) = &mut record.vitals
            {
                vitals.life = life;
                vitals.mana = mana;
                vitals.regen.through = now;
            }
        }
        Ok(())
    }

    fn blight_seconds(
        &self,
        unit: HandleId,
        from: f64,
        now: f64,
        motion: Option<RegenMotion<'_>>,
        work: &mut usize,
    ) -> Result<f64, Error> {
        let fail = |detail| self.inner.error(REGEN, Failure::UnitActivity(detail));
        let blight = self
            .initial_blight
            .as_ref()
            .ok_or_else(|| fail("blight regeneration needs terrain blight state".into()))?;
        let record = self.executed_unit(unit)?;
        let current = record
            .position
            .ok_or_else(|| fail("blight regeneration needs accepted position".into()))?;
        let Some((spans, starts)) = motion else {
            if record.activity.as_ref().is_some_and(|a| a.moving()) {
                return Err(fail(
                    "blight regeneration needs motion projected before a clock rebase".into(),
                ));
            }
            return blight
                .seconds(current, current, now - from, work)
                .map_err(fail);
        };
        let mut position = starts
            .binary_search_by_key(&unit, |(id, _)| *id)
            .ok()
            .map(|i| starts[i].1)
            .unwrap_or(current);
        let begin = spans.partition_point(|s| s.unit < unit);
        let end = spans.partition_point(|s| s.unit <= unit);
        let mut cursor = from;
        let mut total = 0.0;
        for span in &spans[begin..end] {
            if span.end <= from {
                continue;
            }
            if span.start > now || span.end > now || span.end <= span.start {
                return Err(fail(
                    "blight motion span is outside the regeneration interval".into(),
                ));
            }
            let start = span.start.max(from);
            if start < cursor {
                return Err(fail("blight motion spans overlap".into()));
            }
            total += blight
                .seconds(position, position, start - cursor, work)
                .map_err(fail)?;
            let fraction = (start - span.start) / (span.end - span.start);
            let clipped = std::array::from_fn(|i| {
                (f64::from(span.from[i])
                    + (f64::from(span.to[i]) - f64::from(span.from[i])) * fraction)
                    as f32
            });
            total += blight
                .seconds(clipped, span.to, span.end - start, work)
                .map_err(fail)?;
            cursor = span.end;
            position = span.to;
        }
        if position != current {
            return Err(fail(
                "blight regeneration needs relocation reconciled at the projection boundary".into(),
            ));
        }
        total += blight
            .seconds(position, position, now - cursor, work)
            .map_err(fail)?;
        Ok(total)
    }

    /// Base life, mana and unconditional bonus life rates. Base life eligibility
    /// does not gate hero strength or held-item regeneration. A held mana fraction
    /// scales the base plus intelligence rate.
    fn regen_rates(&self, unit: HandleId, bonuses: &HeroVitals) -> Result<(f64, f64, f64), Error> {
        let record = self.executed_unit(unit)?;
        let vitals = record.vitals.expect("filtered to vitals");
        let life = f64::from(vitals.regen.life_per_second);
        let mut mana = f64::from(vitals.regen.mana_per_second);
        let bonus = self.unit_hero_bonus(unit)?;
        let mut bonus_life = f64::from(bonus.life_regen);
        if record.definition.rawcode[0].is_ascii_uppercase() {
            let progress = self.hero_initial_level().zip(
                record
                    .abilities
                    .as_ref()
                    .and_then(|a| a.hero.as_ref())
                    .map(|h| h.level),
            );
            let derive = |name: &str| {
                Self::derive_hero_attribute(&record.definition, progress, bonus, name)
                    .map_err(|e| self.inner.error(REGEN, Failure::UnitActivity(e)))
            };
            bonus_life += f64::from(derive("STR")?) * f64::from(bonuses.life_regen_per_strength);
            mana += f64::from(derive("INT")?) * f64::from(bonuses.mana_regen_per_intelligence);
        }
        let mana_fraction = f64::from(bonus.mana_regen_fraction);
        mana *= 1.0 + mana_fraction;
        if !(life.is_finite()
            && life >= 0.0
            && mana.is_finite()
            && mana >= 0.0
            && mana_fraction.is_finite()
            && mana_fraction >= 0.0
            && bonus_life.is_finite()
            && bonus_life >= 0.0)
        {
            return Err(self.inner.error(
                REGEN,
                Failure::UnitActivity("invalid regeneration rate".into()),
            ));
        }
        Ok((life, mana, bonus_life))
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
                    Entry::RemovedUnit { .. } => return Ok(Value::Real(0.0)),
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
        regen: old.regen,
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
        let regen = Regen {
            life_per_second: 0.25,
            mana_per_second: 0.01,
            life_type: RegenType::Always,
            through: 0.0,
        };
        let old = Vitals {
            life: 40.0,
            max_life: 100.0,
            mana: 7.0,
            max_mana: 40.0,
            regen,
        };
        let next = adjust_hero_vitals(old, 25.0, 15.0).unwrap();
        assert_eq!(
            next,
            Vitals {
                life: 65.0,
                max_life: 125.0,
                mana: 22.0,
                max_mana: 55.0,
                regen,
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
                max_mana: 30.0,
                regen,
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

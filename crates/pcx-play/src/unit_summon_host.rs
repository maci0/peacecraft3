//! Water Elemental uses the selected ability row and the ordinary unit factory.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::{HandleId, Value};
use std::collections::BTreeMap;

const ABILITY: [u8; 4] = *b"AHwe";
const NATIVE: &str = "IssueImmediateOrder";

pub(super) struct State {
    order: i32,
    pub(super) ready: BTreeMap<HandleId, f64>,
    pending: BTreeMap<HandleId, f64>,
    expires: BTreeMap<HandleId, f64>,
    animations: BTreeMap<HandleId, (usize, f64, f64)>,
}

struct Spell {
    cost: f32,
    area: f32,
    cooldown: f64,
    duration: f64,
    count: usize,
    unit: [u8; 4],
}

impl ObjectHost {
    fn summon_error(&self, detail: impl Into<String>) -> Error {
        self.inner
            .error(NATIVE, Failure::UnitActivity(detail.into()))
    }

    pub fn admit_water_elemental(&mut self) -> Result<(), Error> {
        if self.summoning.is_some() || self.combat.is_none() || !self.has_hero_skills() {
            return Err(
                self.summon_error("summoning requires combat and hero skills, admitted once")
            );
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateUnit")
            .expect("admitted units")
            .source;
        for name in ["GetSummoningUnit", "GetSummonedUnit"] {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract_in(&self.inner, name, &[], "unit", true, Some("unit"), false)?;
                if native.source != *source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        let name = self
            .hero_ability_profile(ABILITY, "Order")
            .map_err(|e| self.summon_error(e))?;
        let order = self
            .order_registry
            .as_ref()
            .and_then(|r| r.id(&name))
            .filter(|id| *id != -1)
            .ok_or_else(|| {
                self.summon_error(format!("summoning order {name:?} is not registered"))
            })?;
        self.summoning = Some(State {
            order,
            ready: BTreeMap::new(),
            pending: BTreeMap::new(),
            expires: BTreeMap::new(),
            animations: BTreeMap::new(),
        });
        Ok(())
    }

    /// The learned ability's source hotkey and registered immediate order.
    pub fn water_elemental_command(
        &self,
        unit: HandleId,
    ) -> Result<Option<super::holy_light::Command>, Error> {
        let Some(state) = &self.summoning else {
            return Ok(None);
        };
        if self.summon_spell(unit)?.is_none() {
            return Ok(None);
        }
        let hotkey = self
            .hero_ability_profile(ABILITY, "Hotkey")
            .map_err(|e| self.summon_error(e))?;
        match hotkey.as_bytes() {
            [letter] if letter.is_ascii_alphabetic() => Ok(Some(super::holy_light::Command {
                ability: ABILITY,
                hotkey: char::from(letter.to_ascii_uppercase()),
                order: state.order,
            })),
            _ => Err(self.summon_error(format!("summoning Hotkey {hotkey:?} is not one letter"))),
        }
    }

    /// Current source-ranked cost and live cooldown; target/creation checks remain separate.
    pub fn water_elemental_readiness(
        &self,
        unit: HandleId,
    ) -> Result<Option<crate::ability_profile::Readiness>, Error> {
        let Some(state) = self.summoning.as_ref() else {
            return Ok(None);
        };
        let Some(spell) = self.summon_spell(unit)? else {
            return Ok(None);
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.summon_error("clock missing"))?;
        Ok(Some(Self::ability_readiness_at(
            self.executed_unit(unit)?,
            spell.cost,
            state.ready.get(&unit).copied(),
            now,
        )))
    }

    fn track_summon_animation(&mut self, unit: HandleId) -> Result<(), Error> {
        let Some(animation) = self.executed_unit(unit)?.animation.as_ref() else {
            return Ok(());
        };
        let Some(index) = animation.sequence else {
            return Ok(());
        };
        let started = animation.started;
        let end = started + f64::from(animation.model.sequences[index].duration()) / 1000.;
        self.summoning
            .as_mut()
            .expect("admitted")
            .animations
            .insert(unit, (index, started, end));
        Ok(())
    }

    fn finish_summon_animations(&mut self, now: f64) -> Result<(), Error> {
        let due: Vec<_> = self
            .summoning
            .as_ref()
            .expect("admitted")
            .animations
            .iter()
            .filter(|(_, (_, _, end))| now >= *end)
            .map(|(&unit, &state)| (unit, state))
            .collect();
        for (unit, (index, started, _)) in due {
            self.summoning
                .as_mut()
                .expect("admitted")
                .animations
                .remove(&unit);
            if let Entry::Unit { record } = self.inner.entry_mut(unit)?
                && let Some(animation) = &mut record.animation
                && animation.sequence == Some(index)
                && animation.started == started
            {
                animation
                    .select("stand", now, None)
                    .map_err(|e| self.summon_error(e))?;
            }
        }
        Ok(())
    }

    fn summon_spell(&self, caster: HandleId) -> Result<Option<Spell>, Error> {
        let Some(unit) = self.live(caster) else {
            return Ok(None);
        };
        if !self.unit_ability_available(caster, ABILITY)? {
            return Ok(None);
        }
        let rank = unit
            .abilities
            .as_ref()
            .map_or(0, |abilities| abilities.level(ABILITY));
        if rank <= 0 {
            return Ok(None);
        }
        let levels = self
            .hero_ability_number(ABILITY, "levels")
            .map_err(|e| self.summon_error(e))?;
        if rank > levels {
            return Err(self.summon_error("summoning rank exceeds selected ability levels"));
        }
        let text = |stem, kind| {
            self.hero_ability_level_cell(ABILITY, stem, rank, kind)
                .map_err(|e| self.summon_error(e))
        };
        let number = |stem, kind| -> Result<f64, Error> {
            let value = text(stem, kind)?
                .parse::<f32>()
                .map_err(|_| self.summon_error(format!("invalid summoning {stem}{rank}")))?;
            if !value.is_finite() || value < 0. {
                return Err(self.summon_error(format!(
                    "summoning {stem}{rank} must be finite and nonnegative"
                )));
            }
            Ok(f64::from(value))
        };
        let rawcode = text("UnitID", FieldKind::Token)?;
        let unit = rawcode
            .as_bytes()
            .try_into()
            .map_err(|_| self.summon_error("summoning UnitID must be a four-byte rawcode"))?;
        let count = text("DataA", FieldKind::Int)?
            .parse::<usize>()
            .map_err(|_| self.summon_error("invalid summoned unit count"))?;
        let cost = number("Cost", FieldKind::Int)? as f32;
        let duration = number("Dur", FieldKind::Unreal)?;
        if count == 0 || duration <= 0. || !cost.is_finite() {
            return Err(
                self.summon_error("summoning requires positive count and lifetime and finite cost")
            );
        }
        Ok(Some(Spell {
            cost,
            area: number("Area", FieldKind::Unreal)? as f32,
            cooldown: f64::from(self.ability_cooldown_duration(caster, ABILITY, rank)?),
            duration,
            count,
            unit,
        }))
    }

    pub(super) fn cancel_summon_cast(&mut self, caster: HandleId) {
        if let Some(state) = &mut self.summoning {
            state.pending.remove(&caster);
        }
    }

    pub(super) fn resume_summon_cast(&mut self, caster: HandleId, elapsed: f64) {
        if let Some(due) = self
            .summoning
            .as_mut()
            .and_then(|s| s.pending.get_mut(&caster))
        {
            *due += elapsed;
        }
    }

    pub(super) fn summoning_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !matches!(name, "IssueImmediateOrder" | "IssueImmediateOrderById") {
            return None;
        }
        let state = self.summoning.as_ref()?;
        let [unit, order] = args else { return None };
        let order = match (name, order) {
            ("IssueImmediateOrder", Value::Str(text)) => self.order_registry.as_ref()?.id(text)?,
            ("IssueImmediateOrderById", Value::Int(id)) => *id,
            _ => return None,
        };
        if order != state.order {
            return None;
        }
        Some((|| {
            let caster = match unit {
                Value::Null => return Ok(Value::Bool(false)),
                Value::Handle(id) => *id,
                _ => return Err(self.summon_error("caster must be a unit")),
            };
            if matches!(self.inner.entry(caster)?, Entry::RemovedUnit) {
                return Ok(Value::Bool(false));
            }
            let Some(spell) = self.summon_spell(caster)? else {
                return Ok(Value::Bool(false));
            };
            let now = self
                .timer_game_time()
                .ok_or_else(|| self.summon_error("summoning requires game time"))?;
            let record = self.executed_unit(caster)?;
            if !Self::ability_readiness_at(
                record,
                spell.cost,
                self.summoning
                    .as_ref()
                    .and_then(|s| s.ready.get(&caster).copied()),
                now,
            )
            .ready()
            {
                return Ok(Value::Bool(false));
            }
            self.check_unit_creation_capacity(spell.count)?;
            let cell = super::combat::column(&record.definition, SlkId::UnitWeapons, "castpt")
                .map_err(|e| self.summon_error(e))?
                .ok_or_else(|| self.summon_error("caster cast point is absent"))?;
            let delay = super::combat::number(cell, "castpt")
                .map_err(|e| self.summon_error(e))?
                .map(f64::from)
                .ok_or_else(|| self.summon_error("caster cast point is absent"))?;
            if !delay.is_finite() || delay < 0. || !(now + delay).is_finite() {
                return Err(self.summon_error("caster cast point is nonfinite or negative"));
            }
            let mut animation = record
                .animation
                .clone()
                .filter(|animation| animation.has_sequence("spell"));
            if let Some(animation) = &mut animation {
                animation
                    .select("spell", now, None)
                    .map_err(|e| self.summon_error(e))?;
            }
            self.steer_unit(caster, None, NATIVE)?;
            self.unit_ordered(caster);
            self.cancel_inventory_order(caster);
            self.clear_combat_target(caster);
            if let Entry::Unit { record } = self.inner.entry_mut(caster)?
                && animation.is_some()
            {
                record.animation = animation;
            }
            if self
                .executed_unit(caster)?
                .animation
                .as_ref()
                .is_some_and(|a| a.has_sequence("spell"))
            {
                self.track_summon_animation(caster)?;
            }
            self.summoning
                .as_mut()
                .expect("admitted")
                .pending
                .insert(caster, now + delay);
            Ok(Value::Bool(true))
        })())
    }

    pub(super) fn summon_current_order(&self, caster: HandleId) -> Option<i32> {
        self.summoning
            .as_ref()
            .filter(|s| s.pending.contains_key(&caster))
            .map(|s| s.order)
    }

    pub(super) fn advance_summoning(&mut self, now: f64) -> Result<(), Error> {
        let Some(state) = &self.summoning else {
            return Ok(());
        };
        let expired: Vec<_> = state
            .expires
            .iter()
            .filter(|(_, due)| **due <= now)
            .map(|(unit, _)| *unit)
            .collect();
        for unit in expired {
            self.summoning
                .as_mut()
                .expect("admitted")
                .expires
                .remove(&unit);
            self.kill_unit(unit, None)?;
        }
        self.finish_summon_animations(now)?;
        let due: Vec<_> = self
            .summoning
            .as_ref()
            .expect("admitted")
            .pending
            .iter()
            .filter(|(_, due)| **due <= now)
            .map(|(caster, _)| *caster)
            .collect();
        for caster in due {
            let Some(record) = self.live(caster) else {
                self.cancel_summon_cast(caster);
                continue;
            };
            if record.paused_since.is_some() {
                continue;
            }
            let Some(spell) = self.summon_spell(caster)? else {
                self.cancel_summon_cast(caster);
                continue;
            };
            let record = self.executed_unit(caster)?;
            if record.vitals.is_none_or(|v| v.mana < spell.cost) {
                self.cancel_summon_cast(caster);
                continue;
            }
            let (owner, face, position) = (
                record.owner,
                record.face,
                record
                    .position
                    .ok_or_else(|| self.summon_error("caster position is absent"))?,
            );
            let (sin, cos) = face.to_radians().sin_cos();
            let origin = [
                position[0] + spell.area * cos,
                position[1] + spell.area * sin,
            ];
            if !origin.into_iter().all(f32::is_finite) {
                return Err(self.summon_error("summoning placement is nonfinite"));
            }
            self.check_unit_creation_capacity(spell.count)?;
            if !(now + spell.duration).is_finite() || !(now + spell.cooldown).is_finite() {
                return Err(self.summon_error("summoning time overflow"));
            }
            let mut spawned = Vec::new();
            spawned
                .try_reserve_exact(spell.count)
                .map_err(|_| self.summon_error("summoning allocation failed"))?;
            for _ in 0..spell.count {
                // shortcut: qualify native collision adjustment of this source-derived request before claiming placement parity.
                let Value::Handle(unit) = self.create_summoned_unit(&[
                    Value::Handle(owner),
                    Value::Int(i32::from_be_bytes(spell.unit)),
                    Value::Real(origin[0]),
                    Value::Real(origin[1]),
                    Value::Real(face),
                ])?
                else {
                    return Err(self.summon_error("unit factory returned no summoned unit"));
                };
                if let Entry::Unit { record } = self.inner.entry_mut(unit)?
                    && let Some(animation) = &mut record.animation
                    && animation.has_sequence("birth")
                {
                    animation
                        .select("birth", now, None)
                        .map_err(|e| self.summon_error(e))?;
                    self.track_summon_animation(unit)?;
                }
                spawned.push(unit);
            }
            if let Entry::Unit { record } = self.inner.entry_mut(caster)?
                && let Some(vitals) = &mut record.vitals
            {
                vitals.mana -= spell.cost;
            }
            {
                let state = self.summoning.as_mut().expect("admitted");
                state.pending.remove(&caster);
                state.ready.insert(caster, now + spell.cooldown);
                for &unit in &spawned {
                    state.expires.insert(unit, now + spell.duration);
                }
            }
            self.queue_spell_effect(caster, None)?;
            for unit in spawned {
                self.queue_ability_event(
                    caster,
                    "EVENT_UNIT_SUMMON",
                    "EVENT_PLAYER_UNIT_SUMMON",
                    super::EventSubject {
                        unit: Some(caster),
                        summon: Some(super::event_delivery::SummonContext {
                            summoner: caster,
                            summoned: unit,
                        }),
                        ..Default::default()
                    },
                )?;
            }
        }
        Ok(())
    }
}

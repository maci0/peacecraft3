//! One corpse lifecycle on the canonical unit; suspension never freezes its animation.
use super::{Entry, Error, Failure, ObjectHost};
use crate::unit_decay::{Kind, Phase, Rules, State};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::Value;
use pcx_sim::world::FieldValue;
const NATIVE: &str = "UnitSuspendDecay";
impl ObjectHost {
    pub fn admit_unit_decay(&mut self, rules: Rules) -> Result<(), Error> {
        self.unit_catalog()?;
        if self.unit_decay.is_some()
            || self.unit_activity.is_none()
            || self.timer_game_time().is_none()
            || self
                .inner
                .arena
                .iter()
                .any(|e| matches!(e, Entry::Unit { .. } | Entry::RemovedUnit { .. }))
            || self
                .player_client_sha256()
                .is_some_and(|sha| sha != rules.image_sha256)
        {
            return Err(self.inner.error(
                NATIVE,
                Failure::UnitState("decay admission requires one client, activity and no units"),
            ));
        }
        if let Some(native) = self.inner.program.natives().get(NATIVE) {
            Self::contract(
                &self.inner,
                NATIVE,
                &["unit", "boolean"],
                "nothing",
                false,
                Some("unit"),
            )?;
            if native.source
                != self
                    .inner
                    .program
                    .natives()
                    .get("CreateUnit")
                    .expect("unit catalog")
                    .source
            {
                return Err(self.inner.error(NATIVE, Failure::Source));
            }
        }
        self.unit_decay = Some(rules);
        Ok(())
    }
    pub(super) fn prepare_unit_decay(
        &self,
        record: &super::ExecutedUnit,
    ) -> Result<Option<State>, Error> {
        let Some(rules) = &self.unit_decay else {
            return Ok(None);
        };
        let read = || -> Result<State, String> {
            let d = &record.definition;
            let value = super::unit_positions::field(
                d,
                SlkId::UnitData,
                "death",
                &[FieldKind::Unreal, FieldKind::Real],
            )
            .map_err(str::to_owned)?;
            let death = match value {
                FieldValue::Cell(v) => v
                    .parse::<f32>()
                    .map_err(|_| "UnitData/death is not a real")?,
                FieldValue::Override(
                    pcx_objects::Value::Real(v) | pcx_objects::Value::Unreal(v),
                ) => *v,
                _ => return Err("UnitData/death wire type".into()),
            };
            if !death.is_finite() || death < 0. {
                return Err("UnitData/death must be finite and nonnegative".into());
            }
            let flags = super::unit_positions::text(d, SlkId::UnitData, "deathType")
                .map_err(str::to_owned)?
                .parse::<u32>()
                .map_err(|_| "UnitData/deathType is not a mask")?;
            if flags & !(rules.raise_mask | rules.decay_mask) != 0 {
                return Err("unsupported UnitData/deathType mask".into());
            }
            let hero = record.abilities.as_ref().map_or_else(
                || (rules.hero_range[0]..=rules.hero_range[1]).contains(&d.rawcode[0]),
                |a| a.hero.is_some(),
            );
            let kind = if hero {
                Kind::Hero
            } else if super::unit_types::structure(d)? {
                Kind::Structure
            } else {
                Kind::Unit
            };
            Ok(State {
                kind,
                death: f64::from(death),
                raises: flags & rules.raise_mask != 0,
                decays: flags & rules.decay_mask != 0,
                phase: Phase::Alive,
                deadline: None,
                remaining: 0.,
                suspended: false,
            })
        };
        read()
            .map(Some)
            .map_err(|e| self.inner.error("unit decay", Failure::UnitAnimation(e)))
    }
    pub(super) fn unit_decay_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != NATIVE
            || self.unit_decay.is_none()
            || self.inner.program.natives().get(name).is_none()
        {
            return None;
        }
        Some((|| {
            let [unit, Value::Bool(suspend)] = args else {
                return Err(self.inner.error(name, Failure::Signature));
            };
            let id = match unit {
                Value::Null => return Ok(Value::Null),
                Value::Handle(id) => *id,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            let mut state = match self
                .inner
                .entry(id)
                .map_err(|e| self.inner.error(name, e.failure))?
            {
                Entry::RemovedUnit { .. } => return Ok(Value::Null),
                Entry::Unit { record } => record.decay.clone(),
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            let Some(decay) = &mut state else {
                return Err(self
                    .inner
                    .error(name, Failure::UnitState("unit decay state absent")));
            };
            if !matches!(decay.phase, Phase::Flesh | Phase::Bone) || decay.suspended == *suspend {
                return Ok(Value::Null);
            }
            let now = self.timer_game_time().expect("decay clock");
            self.check_activity_boundary(now, name)?;
            // shortcut: resume preserves remaining phase time; qualify native restart timing against the original runtime.
            if *suspend {
                decay.remaining = decay.deadline.map_or(0., |at| (at - now).max(0.));
                decay.deadline = None;
            } else {
                decay
                    .schedule(now, decay.remaining)
                    .map_err(|e| self.inner.error(name, Failure::UnitAnimation(e)))?;
            }
            decay.suspended = *suspend;
            let Entry::Unit { record } = self.inner.entry_mut(id)? else {
                unreachable!()
            };
            record.decay = state;
            Ok(Value::Null)
        })())
    }
    pub(super) fn advance_unit_decay(&mut self, now: f64, work: &mut usize) -> Result<(), Error> {
        let Some(rules) = &self.unit_decay else {
            return Ok(());
        };
        let mut plans = Vec::new();
        let mut remaining = *work;
        for (id, record) in self.executed_units() {
            let Some(state) = &record.decay else { continue };
            if state.deadline.is_none_or(|at| at > now) {
                continue;
            }
            let mut state = state.clone();
            let mut animation = record.animation.clone();
            let mut remove = false;
            while let Some(at) = state.deadline.filter(|at| *at <= now) {
                remaining = remaining.checked_sub(1).ok_or_else(|| {
                    self.inner
                        .error("unit decay", Failure::Budget("corpse transitions"))
                })?;
                let mut clip = None;
                match state.phase {
                    Phase::Dying if state.kind == Kind::Hero => {
                        state.phase = Phase::Dissipating;
                        state
                            .schedule(at, rules.dissipate)
                            .map_err(|e| self.decay_error(e))?;
                        clip = Some("dissipate");
                    }
                    Phase::Dying if state.decays => {
                        state.phase = Phase::Flesh;
                        state
                            .schedule(at, rules.flesh)
                            .map_err(|e| self.decay_error(e))?;
                        clip = Some("decay flesh");
                    }
                    Phase::Dying if state.raises => {
                        state.phase = Phase::Remains;
                        state.deadline = None;
                    }
                    Phase::Dying | Phase::Bone => {
                        remove = true;
                        state.deadline = None;
                    }
                    Phase::Flesh => {
                        state.phase = Phase::Bone;
                        state
                            .schedule(
                                at,
                                if state.kind == Kind::Structure {
                                    rules.structure
                                } else {
                                    rules.bone
                                },
                            )
                            .map_err(|e| self.decay_error(e))?;
                        clip = Some("decay bone");
                    }
                    Phase::Dissipating => {
                        state.phase = Phase::Hidden;
                        state.deadline = None;
                    }
                    _ => return Err(self.decay_error("unexpected corpse deadline")),
                }
                if let (Some(clip), Some(animation)) = (clip, &mut animation) {
                    let clip = if animation.has_sequence(clip) {
                        clip
                    } else if clip.starts_with("decay") {
                        "decay"
                    } else {
                        clip
                    };
                    animation
                        .select_for_duration(clip, at, state.remaining)
                        .map_err(|e| self.decay_error(e))?;
                }
            }
            plans
                .try_reserve(1)
                .map_err(|_| self.decay_error("corpse transition storage"))?;
            plans.push((id, state, animation, remove));
        }
        *work = remaining;
        for (id, state, animation, remove) in plans {
            if remove {
                self.remove_unit_canonical(id)
                    .map_err(|e| self.decay_error(e.to_string()))?;
            } else if let Entry::Unit { record } = self.inner.entry_mut(id)? {
                if state.phase == Phase::Hidden {
                    record.shown = false;
                }
                record.decay = Some(state);
                record.animation = animation;
            }
        }
        Ok(())
    }
    fn decay_error(&self, detail: impl Into<String>) -> Error {
        self.inner
            .error("unit decay", Failure::UnitAnimation(detail.into()))
    }
}

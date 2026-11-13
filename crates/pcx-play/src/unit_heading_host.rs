//! Canonical logical/visual heading state on the source physics request.
use super::*;
use crate::{native_game_clock::Point, script_random::real, unit_activity::heading};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Heading {
    pub(crate) state: heading::visual::State,
    pub(crate) speed: f32,
    pub(crate) window: f32,
    pub(crate) selector: usize,
    pub(crate) target: Option<HandleId>,
}

#[derive(Clone, Copy)]
pub(crate) struct Properties {
    speed: f32,
    window: f32,
    selector: usize,
}

impl Heading {
    pub(super) fn properties(self) -> Properties {
        Properties {
            speed: self.speed,
            window: self.window,
            selector: self.selector,
        }
    }
    pub(super) fn set_properties(&mut self, properties: Properties) {
        self.speed = properties.speed;
        self.window = properties.window;
        self.selector = properties.selector;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Ready,
    VisualPending { at: Point },
}

#[derive(Clone, Debug)]
pub(crate) struct Request {
    pub(crate) period: u32,
    pub(crate) next: Point,
    pub(crate) phase: Phase,
    pub(crate) fault: Option<String>,
}

impl ObjectHost {
    fn heading_error(&self, message: impl Into<String>) -> Error {
        self.inner
            .error("unit heading", Failure::UnitActivity(message.into()))
    }

    pub(crate) fn prepare_unit_heading(
        &self,
        definition: &Definition,
        face: f32,
    ) -> Result<Option<Heading>, Error> {
        let Some(rules) = self
            .unit_activity
            .as_ref()
            .and_then(|s| s.rules.heading_rules())
        else {
            return Ok(None);
        };
        let profile = rules
            .profile
            .as_ref()
            .ok_or_else(|| self.heading_error("heading profile source missing"))?;
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| self.heading_error("heading numeric source missing"))?;
        let cell = |name: &str| -> Result<f32, Error> {
            let mut fields = matching_columns(definition, SlkId::UnitData, name);
            let (_, field) = fields
                .next()
                .ok_or_else(|| self.heading_error(format!("missing UnitData/{name}")))?;
            if fields.next().is_some() {
                return Err(self.heading_error(format!("ambiguous UnitData/{name}")));
            }
            let value = match field
                .value
                .as_ref()
                .map_err(|e| self.heading_error(format!("UnitData/{name}: {e:?}")))?
            {
                FieldValue::Cell(text) => profile
                    .authored(numeric, text)
                    .map_err(|e| self.heading_error(e))?,
                FieldValue::Override(pcx_objects::Value::Int(value)) => f32::from_bits(
                    real::from_i32(*value as i32).map_err(|e| self.heading_error(e.to_string()))?,
                ),
                FieldValue::Override(
                    pcx_objects::Value::Real(value) | pcx_objects::Value::Unreal(value),
                ) => *value,
                _ => return Err(self.heading_error(format!("unsupported UnitData/{name}"))),
            };
            if !value.is_finite() || value < profile.zero {
                return Err(
                    self.heading_error(format!("nonnegative finite UnitData/{name} required"))
                );
            }
            Ok(value)
        };
        let speed = cell(&profile.turn_column)?;
        let window = cell(&profile.window_column)?;
        let selector = cell(&profile.selector_column)?;
        if selector.fract() != profile.zero
            || selector > u8::MAX as f32
            || !face.is_finite()
            || face < profile.zero
        {
            return Err(self.heading_error("heading selector or initial angle is unqualified"));
        }
        let logical = rules
            .normalize(
                numeric,
                profile.radians(face).map_err(|e| self.heading_error(e))?,
            )
            .map_err(|e| self.heading_error(e.to_string()))?;
        if speed >= rules.full_turn
            || profile.radians(window).map_err(|e| self.heading_error(e))? >= rules.full_turn
        {
            return Err(self
                .heading_error("multi-revolution property setter normalization is not qualified"));
        }
        let speed = rules
            .normalize(numeric, speed)
            .map_err(|e| self.heading_error(e.to_string()))?
            .max(profile.minimum_turn);
        let window = rules
            .normalize(
                numeric,
                profile.radians(window).map_err(|e| self.heading_error(e))?,
            )
            .map_err(|e| self.heading_error(e.to_string()))?;
        let selector = (selector as u32 & profile.selector_mask) as usize;
        if rules
            .visual
            .as_ref()
            .is_none_or(|v| selector >= v.curves.len())
        {
            return Err(self.heading_error("heading orientation selector is not admitted"));
        }
        Ok(Some(Heading {
            state: heading::visual::State {
                logical,
                angle: logical,
                velocity: profile.zero,
            },
            speed,
            window,
            selector,
            target: None,
        }))
    }

    /// Prepare before a caller changes animation, resources, position or swing state.
    pub(crate) fn prepare_native_unit_facing(
        &self,
        unit: HandleId,
        face: f32,
    ) -> Result<Option<Heading>, Error> {
        if !self.native_clock_driving() {
            return Ok(None);
        }
        let Some(mut current) = self
            .executed_unit(unit)?
            .activity
            .as_ref()
            .and_then(|a| a.heading)
        else {
            return Ok(None);
        };
        let rules = self
            .unit_activity
            .as_ref()
            .and_then(|s| s.rules.heading_rules())
            .ok_or_else(|| self.heading_error("heading source missing"))?;
        let profile = rules
            .profile
            .as_ref()
            .ok_or_else(|| self.heading_error("heading profile missing"))?;
        if face < profile.zero {
            return Err(self.heading_error("negative script heading is not qualified"));
        }
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| self.heading_error("heading numeric source missing"))?;
        current.state.logical = rules
            .normalize(
                numeric,
                profile.radians(face).map_err(|e| self.heading_error(e))?,
            )
            .map_err(|e| self.heading_error(e.to_string()))?;
        Ok(Some(current))
    }

    pub(crate) fn prepare_native_unit_look_at(
        &self,
        unit: HandleId,
        from: [f32; 2],
        to: [f32; 2],
    ) -> Result<Option<Heading>, Error> {
        if !self.native_clock_driving() {
            return Ok(None);
        }
        let Some(mut current) = self
            .executed_unit(unit)?
            .activity
            .as_ref()
            .and_then(|a| a.heading)
        else {
            return Ok(None);
        };
        let rules = self
            .unit_activity
            .as_ref()
            .and_then(|s| s.rules.heading_rules())
            .ok_or_else(|| self.heading_error("heading source missing"))?;
        let profile = rules
            .profile
            .as_ref()
            .ok_or_else(|| self.heading_error("heading profile missing"))?;
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| self.heading_error("heading numeric source missing"))?;
        let vector = [
            f32::from_bits(
                real::sub(to[0].to_bits(), from[0].to_bits())
                    .map_err(|e| self.heading_error(e.to_string()))?,
            ),
            f32::from_bits(
                real::sub(to[1].to_bits(), from[1].to_bits())
                    .map_err(|e| self.heading_error(e.to_string()))?,
            ),
        ];
        current.state.logical = profile
            .direction
            .angle(numeric, vector, rules.full_turn, profile.zero)
            .map_err(|e| self.heading_error(e))?;
        Ok(Some(current))
    }

    pub(crate) fn commit_native_unit_facing(
        record: &mut super::super::ExecutedUnit,
        face: f32,
        heading: Option<Heading>,
    ) {
        if let Some(heading) = heading {
            record
                .activity
                .as_mut()
                .expect("prepared heading activity")
                .heading = Some(heading);
        } else {
            record.face = face;
        }
    }

    pub(crate) fn clear_unit_heading_target(&mut self, unit: HandleId) {
        if let Ok(Entry::Unit { record }) = self.inner.entry_mut(unit)
            && let Some(heading) = record.activity.as_mut().and_then(|a| a.heading.as_mut())
        {
            heading.target = None;
        }
    }

    pub(crate) fn prepare_native_heading_target(
        &self,
        unit: HandleId,
        target: HandleId,
    ) -> Result<Option<(Heading, bool)>, Error> {
        if !self.native_clock_driving() {
            return Ok(None);
        }
        let record = self.executed_unit(unit)?;
        let Some(mut heading) = record.activity.as_ref().and_then(|a| a.heading) else {
            return Ok(None);
        };
        if record.activity.as_ref().is_some_and(|a| a.turn.is_some()) {
            return Err(
                self.heading_error("spell facing during a script timed turn is not qualified")
            );
        }
        let from = record
            .position
            .ok_or_else(|| self.heading_error("heading source position missing"))?;
        let to = self
            .executed_unit(target)?
            .position
            .ok_or_else(|| self.heading_error("heading target position missing"))?;
        let rules = self
            .unit_activity
            .as_ref()
            .and_then(|s| s.rules.heading_rules())
            .ok_or_else(|| self.heading_error("heading rules missing"))?;
        let profile = rules
            .profile
            .as_ref()
            .ok_or_else(|| self.heading_error("heading profile missing"))?;
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| self.heading_error("heading numeric source missing"))?;
        let vector = [
            f32::from_bits(
                real::sub(to[0].to_bits(), from[0].to_bits())
                    .map_err(|e| self.heading_error(e.to_string()))?,
            ),
            f32::from_bits(
                real::sub(to[1].to_bits(), from[1].to_bits())
                    .map_err(|e| self.heading_error(e.to_string()))?,
            ),
        ];
        let ready = if profile
            .direction
            .coincident(vector)
            .map_err(|e| self.heading_error(e))?
        {
            true
        } else {
            let goal = profile
                .direction
                .angle(numeric, vector, rules.full_turn, profile.zero)
                .map_err(|e| self.heading_error(e))?;
            rules
                .cast_ready(heading.state.logical, goal)
                .map_err(|e| self.heading_error(e.to_string()))?
        };
        heading.target = (!ready).then_some(target);
        Ok(Some((heading, ready)))
    }

    pub(crate) fn commit_native_heading_target(&mut self, unit: HandleId, prepared: Heading) {
        let Entry::Unit { record } = self.inner.entry_mut(unit).expect("prepared heading unit")
        else {
            unreachable!("prepared heading unit")
        };
        record
            .activity
            .as_mut()
            .expect("prepared heading activity")
            .heading = Some(prepared);
    }

    pub(crate) fn native_heading_waiting(&self, unit: HandleId) -> bool {
        self.native_clock_driving()
            && self
                .executed_unit(unit)
                .ok()
                .and_then(|r| r.activity.as_ref())
                .and_then(|a| a.heading)
                .is_some_and(|h| h.target.is_some())
    }

    pub(crate) fn native_unit_facing(&self, unit: HandleId) -> Result<Option<f32>, Error> {
        if !self.native_clock_driving() {
            return Ok(None);
        }
        let record = self.executed_unit(unit)?;
        let Some(heading) = record.activity.as_ref().and_then(|a| a.heading) else {
            return Ok(None);
        };
        let profile = self
            .unit_activity
            .as_ref()
            .and_then(|s| s.rules.heading_rules())
            .and_then(|r| r.profile.as_ref())
            .ok_or_else(|| self.heading_error("heading profile missing"))?;
        profile
            .degrees(heading.state.angle)
            .map(Some)
            .map_err(|e| self.heading_error(e))
    }

    pub(crate) fn prepare_native_physics_admission(&self) -> Result<Option<Request>, Error> {
        let Some(heading) = self
            .unit_activity
            .as_ref()
            .and_then(|s| s.rules.heading_rules())
        else {
            return Ok(None);
        };
        let clock = self
            .native_game_clock
            .as_ref()
            .ok_or_else(|| self.heading_error("physics source clock missing"))?;
        if self
            .unit_activity
            .as_ref()
            .is_some_and(|s| s.physics.is_some())
        {
            return Err(self.heading_error("physics request already admitted"));
        }
        let period = clock
            .rules()
            .simple_period(heading.period)
            .map_err(|e| self.heading_error(e))?;
        let next = clock
            .rules()
            .periodic_deadline(clock.completed_point(), period)
            .map_err(|e| self.heading_error(e))?;
        Ok(Some(Request {
            period,
            next,
            phase: Phase::Ready,
            fault: None,
        }))
    }

    pub(crate) fn commit_native_physics_admission(&mut self, request: Option<Request>) {
        if let Some(state) = &mut self.unit_activity {
            state.physics = request;
        }
    }

    pub(crate) fn prepare_native_physics_rebase(
        &self,
        work: &mut usize,
    ) -> Result<Option<Request>, Error> {
        let Some(request) = self.unit_activity.as_ref().and_then(|s| s.physics.as_ref()) else {
            return Ok(None);
        };
        if request.phase != Phase::Ready || request.fault.is_some() {
            return Err(
                self.heading_error("physics request cannot rebase in a pending or failed phase")
            );
        }
        *work = work
            .checked_sub(1)
            .ok_or_else(|| self.heading_error("physics rebase work exhausted"))?;
        let clock = self.native_game_clock.as_ref().expect("physics clock");
        if request.next.cycle != clock.point().cycle
            || !f32::from_bits(request.next.local).is_normal()
            || real::below_or_equal(request.next.local, clock.rules().simple.cycle.to_bits())
        {
            return Err(self.heading_error("physics rebase requires an undelivered future request"));
        }
        let mut prepared = request.clone();
        prepared.next = clock
            .rules()
            .rebase(request.next)
            .map_err(|e| self.heading_error(e))?;
        Ok(Some(prepared))
    }

    pub(crate) fn next_native_physics_boundary(
        &self,
        until: Point,
    ) -> Result<Option<Point>, Error> {
        if !self.native_clock_driving() {
            return Ok(None);
        }
        let Some(request) = self.unit_activity.as_ref().and_then(|s| s.physics.as_ref()) else {
            return Ok(None);
        };
        if let Some(fault) = &request.fault {
            return Err(self.heading_error(fault.clone()));
        }
        if request.phase != Phase::Ready {
            return Err(self.heading_error("physics visual phase is still pending"));
        }
        let local = f32::from_bits(request.next.local);
        if request.next.cycle != until.cycle || !local.is_normal() || local.is_sign_negative() {
            return Err(self.heading_error("physics queue point differs"));
        }
        Ok(real::below_or_equal(request.next.local, until.local).then_some(request.next))
    }

    pub(crate) fn pending_native_physics(&self) -> Option<Point> {
        match self.unit_activity.as_ref()?.physics.as_ref()?.phase {
            Phase::VisualPending { at } => Some(at),
            Phase::Ready => None,
        }
    }

    pub(crate) fn check_native_physics_fault(&self) -> Result<(), Error> {
        if let Some(fault) = self
            .unit_activity
            .as_ref()
            .and_then(|s| s.physics.as_ref())
            .and_then(|request| request.fault.as_ref())
        {
            return Err(self.heading_error(fault.clone()));
        }
        Ok(())
    }

    pub(crate) fn native_physics_seconds(&self, point: Point) -> f64 {
        self.native_game_clock
            .as_ref()
            .expect("physics source clock")
            .rules()
            .seconds(point)
    }

    pub(crate) fn fail_native_physics(&mut self, message: String) {
        if let Some(request) = self.unit_activity.as_mut().and_then(|s| s.physics.as_mut())
            && matches!(request.phase, Phase::VisualPending { .. })
        {
            request.fault = Some(message);
        }
    }

    fn physics_fault(&mut self, error: &Error) {
        if let Some(request) = self.unit_activity.as_mut().and_then(|s| s.physics.as_mut()) {
            request.fault = Some(error.to_string());
        }
    }

    pub(crate) fn begin_native_physics(
        &mut self,
        at: Point,
        work: &mut usize,
    ) -> Result<(), Error> {
        if self.next_native_physics_boundary(at)? != Some(at) {
            return Err(self.heading_error("physics request is not due"));
        }
        let now = self
            .native_game_clock
            .as_ref()
            .expect("physics clock")
            .rules()
            .seconds(at);
        self.validate_timer_target(now)?;
        self.begin_native_queue_callback(at)?;
        self.unit_activity
            .as_mut()
            .expect("physics activity")
            .physics
            .as_mut()
            .expect("physics request")
            .phase = Phase::VisualPending { at };
        let result = (|| {
            self.set_timer_game_time(now)?;
            self.advance_unit_activity_mandatory(now, work)?;
            let mut updates = Vec::new();
            for (unit, record) in self.executed_units() {
                *work = work
                    .checked_sub(1)
                    .ok_or_else(|| self.heading_error("physics logical work exhausted"))?;
                let Some(mut heading) = record.activity.as_ref().and_then(|a| a.heading) else {
                    continue;
                };
                let Some(target) = heading.target else {
                    continue;
                };
                if record.paused_since.is_some() {
                    continue;
                }
                if record
                    .activity
                    .as_ref()
                    .is_some_and(|a| a.turn.is_some() || a.motion.is_some())
                {
                    return Err(self.heading_error(
                        "stationary heading overlaps an unqualified movement or script timed turn",
                    ));
                }
                let target_position = self.executed_unit(target).ok().and_then(|t| t.position);
                let Some(to) = target_position else {
                    heading.target = None;
                    updates
                        .try_reserve(1)
                        .map_err(|_| self.heading_error("physics logical storage"))?;
                    updates.push((unit, heading, true));
                    continue;
                };
                let from = record
                    .position
                    .ok_or_else(|| self.heading_error("heading caster position missing"))?;
                let rules = self
                    .unit_activity
                    .as_ref()
                    .and_then(|s| s.rules.heading_rules())
                    .expect("physics heading rules");
                let numeric = self
                    .numeric
                    .as_ref()
                    .ok_or_else(|| self.heading_error("heading numeric source missing"))?;
                let profile = rules
                    .profile
                    .as_ref()
                    .ok_or_else(|| self.heading_error("heading profile source missing"))?;
                let x = f32::from_bits(
                    real::sub(to[0].to_bits(), from[0].to_bits())
                        .map_err(|e| self.heading_error(e.to_string()))?,
                );
                let y = f32::from_bits(
                    real::sub(to[1].to_bits(), from[1].to_bits())
                        .map_err(|e| self.heading_error(e.to_string()))?,
                );
                let goal = rules
                    .normalize(
                        numeric,
                        profile
                            .direction
                            .angle(numeric, [x, y], rules.full_turn, profile.zero)
                            .map_err(|e| self.heading_error(e))?,
                    )
                    .map_err(|e| self.heading_error(e.to_string()))?;
                let step = rules
                    .stationary_step(numeric, heading.state.logical, goal, heading.speed)
                    .map_err(|e| self.heading_error(e))?;
                heading.state.logical = step.logical;
                if step.arrived {
                    heading.target = None;
                }
                updates
                    .try_reserve(1)
                    .map_err(|_| self.heading_error("physics logical storage"))?;
                updates.push((unit, heading, step.arrived));
            }
            for &(unit, heading, _) in &updates {
                let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
                    unreachable!("prepared heading unit")
                };
                record.activity.as_mut().expect("prepared activity").heading = Some(heading);
            }
            // shortcut: canonical arena order breaks equal-frame ties until native cluster order is qualified.
            for (unit, _, arrived) in updates {
                if arrived {
                    self.resume_slow_heading(unit, now)?;
                }
            }
            Ok(())
        })();
        if let Err(error) = &result {
            self.physics_fault(error);
        }
        result
    }

    pub(crate) fn finish_native_physics(
        &mut self,
        at: Point,
        work: &mut usize,
    ) -> Result<(), Error> {
        let request = self
            .unit_activity
            .as_ref()
            .and_then(|s| s.physics.as_ref())
            .ok_or_else(|| self.heading_error("physics request missing"))?;
        if let Some(error) = &request.fault {
            return Err(self.heading_error(error.clone()));
        }
        if request.phase != (Phase::VisualPending { at }) || self.native_game_point() != Some(at) {
            return Err(self.heading_error("physics visual continuation differs"));
        }
        let next = self
            .native_game_clock
            .as_ref()
            .expect("physics clock")
            .rules()
            .periodic_deadline(at, request.period)
            .map_err(|e| self.heading_error(e))?;
        let result = (|| {
            let rules = self
                .unit_activity
                .as_ref()
                .and_then(|s| s.rules.heading_rules())
                .expect("physics heading source");
            let numeric = self
                .numeric
                .as_ref()
                .ok_or_else(|| self.heading_error("heading numeric source missing"))?;
            let visual = rules
                .visual
                .as_ref()
                .ok_or_else(|| self.heading_error("heading visual source missing"))?;
            let profile = rules
                .profile
                .as_ref()
                .ok_or_else(|| self.heading_error("heading profile source missing"))?;
            let mut updates = Vec::new();
            for (unit, record) in self.executed_units() {
                *work = work
                    .checked_sub(1)
                    .ok_or_else(|| self.heading_error("physics visual work exhausted"))?;
                let Some(mut heading) = record.activity.as_ref().and_then(|a| a.heading) else {
                    continue;
                };
                if record.activity.as_ref().is_some_and(|a| a.turn.is_some()) {
                    return Err(self.heading_error(
                        "native orientation during script timed turn is not qualified",
                    ));
                }
                heading.state = visual
                    .step(rules, numeric, heading.state, heading.selector)
                    .map_err(|e| self.heading_error(e))?;
                let face = profile
                    .degrees(heading.state.angle)
                    .map_err(|e| self.heading_error(e))?;
                updates
                    .try_reserve(1)
                    .map_err(|_| self.heading_error("physics visual storage"))?;
                updates.push((unit, heading, face));
            }
            for (unit, heading, face) in updates {
                let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
                    unreachable!("prepared heading unit")
                };
                record
                    .activity
                    .as_mut()
                    .expect("prepared heading activity")
                    .heading = Some(heading);
                record.face = face;
            }
            let request = self
                .unit_activity
                .as_mut()
                .expect("physics activity")
                .physics
                .as_mut()
                .expect("physics request");
            request.next = next;
            request.phase = Phase::Ready;
            self.end_native_queue_callback()
        })();
        if let Err(error) = &result {
            self.physics_fault(error);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity() -> UnitActivity {
        UnitActivity {
            held: false,
            movable: true,
            move_speed: MoveSpeed::new(Some(173.), None),
            motion: None,
            patrol: None,
            follow: None,
            turn: None,
            heading: Some(Heading {
                state: heading::visual::State {
                    logical: 1.3,
                    angle: 0.7,
                    velocity: -0.125,
                },
                speed: 0.4,
                window: 0.3,
                selector: 5,
                target: Some(HandleId::new(std::num::NonZeroU64::new(47).unwrap())),
            }),
            pathing_off: false,
            rally: None,
        }
    }

    #[test]
    fn form_heading_profile_preserves_dynamic_state_and_restores_properties() {
        let mut original = activity();
        let before = original.clone();
        let retained = original.form_profile();
        let mut form = activity();
        form.movable = false;
        form.move_speed = MoveSpeed::new(Some(89.), None);
        let replacement = form.heading.as_mut().unwrap();
        replacement.speed = 0.2;
        replacement.window = 0.8;
        replacement.selector = 2;
        replacement.state.logical = 4.5;
        replacement.target = None;
        original.validate_form_profile(&form).unwrap();
        original.replace_form_profile(&form);
        let heading = original.heading.unwrap();
        assert_eq!(heading.state, before.heading.unwrap().state);
        assert_eq!(heading.target, before.heading.unwrap().target);
        assert_eq!(
            (heading.speed, heading.window, heading.selector),
            (0.2, 0.8, 2)
        );
        assert!(!original.movable);
        original.restore_form_profile(retained);
        assert_eq!(original, before);
        let mut missing = form.clone();
        missing.heading = None;
        assert!(original.validate_form_profile(&missing).is_err());
        assert!(missing.validate_form_profile(&original).is_err());
        assert_eq!(original, before);
    }
}

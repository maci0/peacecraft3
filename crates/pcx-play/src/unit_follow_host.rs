use super::*;

impl ObjectHost {
    pub(crate) fn follow_position(&self, unit: HandleId) -> Option<[f32; 2]> {
        let record = self.executed_unit(unit).ok()?;
        (record.shown
            && record.combat.as_ref().is_none_or(|c| !c.dead)
            && record.vitals.is_none_or(|v| v.life > 0.0))
        .then_some(record.position)
        .flatten()
    }

    pub(crate) fn cancel_unit_follow(&mut self, unit: HandleId) {
        if let Ok(Entry::Unit { record }) = self.inner.entry_mut(unit)
            && let Some(activity) = &mut record.activity
        {
            activity.follow = None;
        }
    }

    pub(crate) fn follow_target_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !matches!(name, "IssueTargetOrder" | "IssueTargetOrderById") {
            return None;
        }
        self.inner.program.natives().get(name)?;
        let state = self.unit_activity.as_ref()?;
        let [unit, order, target] = args else {
            return Some(Err(self.inner.error(name, Failure::Signature)));
        };
        let order = match (name, order) {
            ("IssueTargetOrder", Value::Str(s)) => self.order_registry.as_ref()?.id(s),
            ("IssueTargetOrderById", Value::Int(id)) => Some(*id),
            _ => return Some(Err(self.inner.error(name, Failure::Signature))),
        };
        if order != Some(state.rules.movement) {
            return None;
        }
        Some((|| {
            let (Value::Handle(unit), Value::Handle(target)) = (unit, target) else {
                return Ok(Value::Bool(false));
            };
            if unit == target
                || self.follow_position(*unit).is_none()
                || self.follow_position(*target).is_none()
            {
                return Ok(Value::Bool(false));
            }
            let record = self.executed_unit(*unit)?;
            if record.paused_since.is_some()
                || record
                    .activity
                    .as_ref()
                    .is_none_or(|a| !a.movable || a.speed.is_none_or(|speed| speed <= 0.0))
            {
                return Ok(Value::Bool(false));
            }
            let fail = |s: &str| self.inner.error(name, Failure::UnitActivity(s.into()));
            let rules = self
                .unit_activity
                .as_ref()
                .expect("activity")
                .rules
                .follow
                .as_ref()
                .ok_or_else(|| fail("follow ranges not admitted"))?;
            let structure =
                super::super::unit_types::structure(&self.executed_unit(*target)?.definition)
                    .map_err(|e| fail(&e))?;
            let follow = Follow {
                target: *target,
                range: if structure {
                    rules.structure_range
                } else {
                    rules.range
                },
                observed: self.follow_position(*target).expect("validated"),
            };
            let now = self
                .timer_game_time()
                .ok_or_else(|| fail("clock not admitted"))?;
            let mut work = self.unit_activity.as_ref().expect("activity").route_work;
            let motion = self.prepare_follow_motion(
                *unit,
                self.follow_position(*target).expect("validated"),
                follow.range,
                now,
                &mut work,
            )?;
            let mut animation = self.executed_unit(*unit)?.animation.clone();
            if let Some(animation) = &mut animation {
                animation
                    .select(
                        if motion.is_some() { "walk" } else { "stand" },
                        now,
                        if motion.is_some() {
                            record.activity.as_ref().and_then(|a| a.speed)
                        } else {
                            None
                        },
                    )
                    .map_err(|e| self.inner.error(name, Failure::UnitAnimation(e)))?;
            }
            let state = self.unit_activity.as_mut().expect("activity");
            state
                .follow_positions
                .try_reserve(state.active.len() + 1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            state
                .active
                .try_reserve(1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            self.unit_ordered(*unit);
            self.cancel_inventory_order(*unit);
            self.clear_combat_target(*unit);
            if let Entry::Unit { record } = self.inner.entry_mut(*unit)? {
                let activity = record.activity.as_mut().expect("validated");
                activity.follow = Some(follow);
                activity.motion = motion;
                activity.held = false;
                record.animation = animation;
            }
            let active = &mut self.unit_activity.as_mut().expect("activity").active;
            if let Err(at) = active.binary_search(unit) {
                active.insert(at, *unit);
            }
            Ok(Value::Bool(true))
        })())
    }

    fn prepare_follow_motion(
        &self,
        unit: HandleId,
        target: [f32; 2],
        range: f32,
        at: f64,
        work: &mut usize,
    ) -> Result<Option<Box<Motion>>, Error> {
        let record = self.executed_unit(unit)?;
        let activity = record.activity.as_ref().expect("activity");
        let position = record.position.ok_or_else(|| {
            self.inner
                .error("follow", Failure::UnitState("accepted position required"))
        })?;
        let gap = (target[0] - position[0]).hypot(target[1] - position[1]);
        if gap <= range {
            return Ok(None);
        }
        let goal = [
            target[0] + (position[0] - target[0]) * (range / gap),
            target[1] + (position[1] - target[1]) * (range / gap),
        ];
        let path = if activity.pathing_off {
            vec![goal]
        } else {
            let radius = self.unit_collision_radius(unit, "follow")?;
            let allowance = self
                .unit_activity
                .as_ref()
                .expect("activity")
                .route_work
                .min(*work);
            let mut remaining = allowance;
            let result = self
                .unit_placement
                .as_ref()
                .expect("admitted placement")
                .route(unit, position, goal, radius, &mut remaining);
            *work -= allowance - remaining;
            result.map_err(|e| {
                self.inner
                    .error("follow", Failure::UnitActivity(e.to_string()))
            })?
        };
        if path.as_slice() == [position] {
            return Ok(None);
        }
        Ok(Some(Box::new(Motion {
            goal,
            path,
            next: 0,
            base: position,
            prefix: 0.0,
            last_distance: 0.0,
            started: at,
        })))
    }

    pub(crate) fn refresh_unit_follow(
        &mut self,
        unit: HandleId,
        target: Option<[f32; 2]>,
        work: &mut usize,
    ) -> Result<(), Error> {
        let record = self.executed_unit(unit)?;
        if record.paused_since.is_some() {
            return Ok(());
        }
        let activity = record.activity.as_ref().expect("activity");
        let Some(follow) = activity.follow else {
            return Ok(());
        };
        let at = self
            .unit_activity
            .as_ref()
            .expect("activity")
            .projected_through;
        // A stationary target keeps the route epoch; moving targets are sampled at the projection boundary.
        if let Some(target) = target
            && target == follow.observed
        {
            let position = record.position.expect("accepted follow position");
            let within = (target[0] - position[0]).hypot(target[1] - position[1]) <= follow.range;
            let unchanged = if within {
                !activity.moving()
            } else {
                activity.moving()
            };
            if unchanged {
                return Ok(());
            }
        }
        let motion = match target {
            Some(target) => self.prepare_follow_motion(unit, target, follow.range, at, work)?,
            None => None,
        };
        let mut animation = record.animation.clone();
        if activity.moving() != motion.is_some()
            && let Some(animation) = &mut animation
        {
            animation
                .select(
                    if motion.is_some() { "walk" } else { "stand" },
                    at,
                    if motion.is_some() {
                        activity.speed
                    } else {
                        None
                    },
                )
                .map_err(|e| self.inner.error("follow", Failure::UnitAnimation(e)))?;
        }
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            let activity = record.activity.as_mut().expect("activity");
            activity.motion = motion;
            activity.follow = target.map(|observed| Follow { observed, ..follow });
            record.animation = animation;
        }
        Ok(())
    }
}

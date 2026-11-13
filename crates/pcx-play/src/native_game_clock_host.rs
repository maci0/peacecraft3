//! The published source clock stays with its canonical arena and timer queue.
use super::{Error, Failure, ObjectHost};
use crate::native_game_clock::{
    Domain, Driver, Input, Point, QueueError, Segment, SleepRequest, State,
};
use std::time::Duration;

#[path = "native_sleep_checkpoint.rs"]
mod checkpoint;

impl ObjectHost {
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn test_native_clock_state(&self) -> Option<State> {
        self.native_game_clock.clone()
    }

    pub(crate) fn admit_native_game_clock(&mut self) -> Result<(), Error> {
        let Some(rules) = self.numeric.as_ref().and_then(|p| p.native_game_clock()) else {
            return Ok(());
        };
        let initial = self
            .timer_game_time()
            .filter(|now| now.is_finite())
            .ok_or_else(|| {
                self.inner.error(
                    "native game clock",
                    Failure::GameClock("initial timer clock is missing or invalid".into()),
                )
            })?;
        if self.native_game_clock.is_some()
            || self.inner.timer_slots.iter().any(|&index| {
                matches!(
                    self.inner.arena[index],
                    super::Entry::Timer { run: Some(_), .. }
                        | super::Entry::RegisteredTimedEvent { .. }
                )
            })
        {
            return Err(self.inner.error(
                "native game clock",
                Failure::GameClock(
                    "source queue must be admitted at fresh initialization before timer requests"
                        .into(),
                ),
            ));
        }
        let mut clock = State::new(rules.clone())
            .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))?;
        if initial.to_bits() != f64::from(rules.simple.origin).to_bits() {
            clock
                .select_legacy(true)
                .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))?;
        }
        self.native_game_clock = Some(clock);
        Ok(())
    }

    pub fn native_game_point(&self) -> Option<Point> {
        self.native_game_clock
            .as_ref()
            .filter(|clock| clock.driver() != Driver::Legacy)
            .map(State::point)
    }
    pub fn native_counter_platform(&self) -> bool {
        self.native_game_clock
            .as_ref()
            .is_some_and(State::counter_platform)
    }
    pub fn native_counter_progress_ns(&self) -> Option<u128> {
        self.native_game_clock
            .as_ref()
            .filter(|clock| clock.counter_platform())
            .map(State::counter_progress_ns)
    }
    pub fn native_counter_pending_ns(&self) -> Option<u128> {
        self.native_game_clock
            .as_ref()
            .filter(|clock| clock.counter_platform())
            .map(State::counter_pending_ns)
    }
    pub fn select_native_counter_platform(&mut self) -> Result<bool, Error> {
        let Some(clock) = self.native_game_clock.as_ref() else {
            return Ok(false);
        };
        if clock.driver() == Driver::Legacy {
            return Ok(false);
        }
        if !self.inner.timer_slots.is_empty() || self.parked_thread_count() != 0 {
            return Err(self.inner.error(
                "native platform",
                Failure::GameClock(
                    "counter platform must be selected before script globals".into(),
                ),
            ));
        }
        let fresh = clock.driver() == Driver::Fresh;
        let physics = if fresh {
            self.prepare_native_physics_admission()?
        } else {
            None
        };
        let mut next = clock.clone();
        next.select_counter_platform()
            .map_err(|e| self.inner.error("native platform", Failure::GameClock(e)))?;
        if fresh {
            self.commit_native_physics_admission(physics);
        }
        self.native_game_clock = Some(next);
        Ok(true)
    }

    pub(crate) fn select_legacy_clock_domain(
        &mut self,
        until: f64,
        advances_clock: bool,
    ) -> Result<(), Error> {
        self.validate_timer_target(until)?;
        let advances = advances_clock
            && (self
                .timer_game_time()
                .is_none_or(|now| until.to_bits() != now.to_bits())
                || self.next_timer_deadline().is_some_and(|at| at <= until));
        if let Some(clock) = &mut self.native_game_clock {
            clock
                .select_legacy(advances)
                .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))?;
        }
        Ok(())
    }
    pub(crate) fn begin_native_queue_callback(&mut self, point: Point) -> Result<(), Error> {
        self.begin_native_domain_callback(Domain::Game, point)
    }
    pub(crate) fn begin_native_domain_callback(
        &mut self,
        domain: Domain,
        point: Point,
    ) -> Result<(), Error> {
        if !self.native_clock_driving() {
            return Err(self.inner.error(
                "native game clock",
                Failure::GameClock("native producer is not active".into()),
            ));
        }
        self.native_game_clock
            .as_mut()
            .expect("native clock")
            .enter_domain_callback(domain, point)
            .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))
    }
    pub(crate) fn end_native_queue_callback(&mut self) -> Result<(), Error> {
        self.end_native_domain_callback()
    }
    pub(crate) fn end_native_domain_callback(&mut self) -> Result<(), Error> {
        self.native_game_clock
            .as_mut()
            .expect("native clock")
            .leave_callback()
            .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))
    }
    pub(crate) fn native_clock_driving(&self) -> bool {
        self.native_game_clock
            .as_ref()
            .is_some_and(|clock| clock.driver() == Driver::Native)
    }
    pub(crate) fn native_frame_active(&self) -> bool {
        self.native_game_clock
            .as_ref()
            .is_some_and(State::frame_active)
    }

    pub(crate) fn enqueue_native_input(
        &mut self,
        input: Input,
        work: &mut usize,
    ) -> Result<(), Error> {
        let fail = |e| self.inner.error("native frame", Failure::GameClock(e));
        let clock = self
            .native_game_clock
            .as_ref()
            .ok_or_else(|| fail("source clock is not admitted".into()))?;
        let fresh = clock.driver() == Driver::Fresh;
        let mut next = clock.clone();
        next.enqueue_input(input, work).map_err(fail)?;
        let prepared = if fresh {
            Some((
                self.prepare_native_timer_admission(work)?,
                self.prepare_native_thread_admission(work)?,
                self.prepare_native_physics_admission()?,
            ))
        } else {
            None
        };
        if let Some((timers, threads, physics)) = prepared {
            self.commit_native_timer_rebase(timers);
            self.commit_native_thread_rows(threads);
            self.commit_native_physics_admission(physics);
        }
        self.native_game_clock = Some(next);
        Ok(())
    }

    pub(crate) fn next_native_frame_segment(
        &mut self,
        work: &mut usize,
    ) -> Result<Option<(Domain, Segment)>, Error> {
        let speed = self
            .startup_settings
            .map(|settings| {
                self.inner
                    .symbolic_value(settings.speed, "gamespeed")
                    .map_err(|e| self.inner.error("native counter", e.failure))
                    .and_then(|value| {
                        usize::try_from(value).map_err(|_| {
                            self.inner
                                .error("native counter", Failure::OutsideDomain(value))
                        })
                    })
            })
            .transpose()?;
        let paused = self.game_paused() == Some(true);
        let clock = self.native_game_clock.as_mut().ok_or_else(|| {
            self.inner.error(
                "native frame",
                Failure::GameClock("source clock is not admitted".into()),
            )
        })?;
        clock
            .next_frame_segment_with_settings(work, speed, paused)
            .map_err(|e| {
                self.inner.error(
                    "native frame",
                    match e {
                        QueueError::Budget(stage) => Failure::Budget(stage),
                        QueueError::Invalid(reason) => Failure::GameClock(reason),
                    },
                )
            })
    }
    pub(crate) fn native_game_clock_seconds(&self, point: Point) -> Result<f64, Error> {
        self.native_game_clock
            .as_ref()
            .map(|clock| clock.rules().seconds(point))
            .ok_or_else(|| {
                self.inner.error(
                    "native frame",
                    Failure::GameClock("source clock is not admitted".into()),
                )
            })
    }

    pub(crate) fn finish_native_frame_segment(
        &mut self,
        domain: Domain,
        segment: Segment,
        work: &mut usize,
    ) -> Result<(), Error> {
        if domain == Domain::Game {
            return self.finish_native_game_segment(segment, work);
        }
        let mut next = self
            .native_game_clock
            .as_ref()
            .expect("admitted native frame")
            .clone();
        next.finish_frame_segment(domain, segment)
            .map_err(|e| self.inner.error("native frame", Failure::GameClock(e)))?;
        let threads = if segment.ends_cycle {
            Some(self.prepare_native_thread_rebase(domain, work)?)
        } else {
            None
        };
        if let Some(threads) = threads {
            self.commit_native_thread_rows(threads);
        }
        self.native_game_clock = Some(next);
        Ok(())
    }

    pub fn native_queue_point(&self, domain: Domain) -> Option<Point> {
        self.native_game_clock
            .as_ref()
            .filter(|clock| clock.driver() != Driver::Legacy)
            .map(|clock| clock.queue_point(domain))
    }

    pub fn prepare_native_sleep(
        &self,
        domain: Domain,
        delay: f32,
        work: &mut usize,
    ) -> Result<Option<SleepRequest>, Error> {
        let Some(clock) = self
            .native_game_clock
            .as_ref()
            .filter(|clock| clock.driver() != Driver::Legacy)
        else {
            return Ok(None);
        };
        let supported = match domain {
            Domain::Game => clock.rules().sleep.ai,
            Domain::Real => clock.rules().sleep.script,
        };
        if supported != domain {
            return Err(self.inner.error(
                "native sleep",
                Failure::GameClock("sleep queue family is unsupported".into()),
            ));
        }
        let request = SleepRequest {
            domain,
            delay: delay.to_bits(),
            origin: clock.queue_point(domain),
            epoch: None,
            point: None,
        };
        let request = if clock.driver() == Driver::Native {
            self.activate_native_sleep(request, work)?
        } else {
            *work = work
                .checked_sub(1)
                .ok_or_else(|| self.inner.error("native sleep", Failure::Capacity))?;
            request
        };
        Ok(Some(request))
    }

    pub fn activate_native_sleep(
        &self,
        mut request: SleepRequest,
        work: &mut usize,
    ) -> Result<SleepRequest, Error> {
        let fail = |e| self.inner.error("native sleep", Failure::GameClock(e));
        let rules = self
            .numeric
            .as_ref()
            .and_then(|numeric| numeric.timer_queries())
            .ok_or_else(|| fail("extended sleep policy is not admitted".into()))?;
        if let Some(epoch) = request.epoch {
            if !rules.check_epoch(epoch)
                || request
                    .point
                    .is_none_or(|point| point.local != epoch.entry_deadline().to_bits())
            {
                return Err(fail("native sleep entry identity differs".into()));
            }
            return Ok(request);
        }
        let numeric = self.numeric.as_ref().expect("source query policy");
        let epoch = rules
            .begin_sleep(
                numeric,
                f32::from_bits(request.origin.local),
                f32::from_bits(request.delay),
                *work,
            )
            .map_err(fail)?;
        let remaining = work
            .checked_sub(1)
            .ok_or_else(|| self.inner.error("native sleep", Failure::Capacity))?;
        request.epoch = Some(epoch);
        request.point = Some(Point {
            cycle: request.origin.cycle,
            local: epoch.entry_deadline().to_bits(),
        });
        *work = remaining;
        Ok(request)
    }

    pub fn advance_native_sleep(&self, request: &mut SleepRequest) -> Result<bool, Error> {
        let fail = |e| self.inner.error("native sleep", Failure::GameClock(e));
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| fail("numeric policy missing".into()))?;
        let rules = numeric
            .timer_queries()
            .ok_or_else(|| fail("extended sleep policy missing".into()))?;
        let mut next = *request;
        let mut epoch = next
            .epoch
            .ok_or_else(|| fail("sleep entry is not active".into()))?;
        let point = next
            .point
            .ok_or_else(|| fail("sleep point is not active".into()))?;
        if point.local != epoch.entry_deadline().to_bits() {
            return Err(fail("sleep point and entry differ".into()));
        }
        let terminal = rules.advance_entry(numeric, &mut epoch).map_err(fail)?;
        next.epoch = Some(epoch);
        next.point = Some(Point {
            local: epoch.entry_deadline().to_bits(),
            ..point
        });
        *request = next;
        Ok(terminal)
    }

    pub fn rebase_native_sleep(&self, request: &mut SleepRequest) -> Result<(), Error> {
        let fail = |e| self.inner.error("native sleep", Failure::GameClock(e));
        let clock = self
            .native_game_clock
            .as_ref()
            .ok_or_else(|| fail("native clock missing".into()))?;
        let point = request
            .point
            .ok_or_else(|| fail("sleep point missing".into()))?;
        if point.cycle != clock.queue_point(request.domain).cycle {
            return Err(fail("sleep entry cycle differs".into()));
        }
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| fail("numeric policy missing".into()))?;
        let rules = numeric
            .timer_queries()
            .ok_or_else(|| fail("extended sleep policy missing".into()))?;
        let mut next = *request;
        let mut epoch = next
            .epoch
            .ok_or_else(|| fail("sleep entry is not active".into()))?;
        rules
            .rebase_epoch(numeric, &mut epoch, clock.rules().simple.cycle.to_bits())
            .map_err(fail)?;
        next.point = Some(clock.rules().rebase(point).map_err(fail)?);
        next.epoch = Some(epoch);
        *request = next;
        Ok(())
    }
    pub(crate) fn pace_native_game_clock(
        &mut self,
        elapsed: Duration,
        work: &mut usize,
    ) -> Result<bool, Error> {
        let Some(clock) = self.native_game_clock.as_ref() else {
            return Ok(false);
        };
        let fresh = clock.driver() == Driver::Fresh;
        let mut next = clock.clone();
        next.pace(elapsed)
            .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))?;
        let timers = if fresh {
            self.prepare_native_timer_admission(work)?
        } else {
            Vec::new()
        };
        let physics = if fresh {
            self.prepare_native_physics_admission()?
        } else {
            None
        };
        self.commit_native_timer_rebase(timers);
        if fresh {
            self.commit_native_physics_admission(physics);
        }
        self.native_game_clock = Some(next);
        Ok(true)
    }

    pub(crate) fn next_native_game_segment(
        &mut self,
        work: &mut usize,
    ) -> Result<Option<(Segment, f64)>, Error> {
        let clock = self
            .native_game_clock
            .as_mut()
            .expect("admitted source clock");
        let segment = clock.next_segment(work).map_err(|e| {
            self.inner.error(
                "native game clock",
                match e {
                    QueueError::Budget(stage) => Failure::Budget(stage),
                    QueueError::Invalid(reason) => Failure::GameClock(reason),
                },
            )
        })?;
        Ok(segment.map(|segment| (segment, clock.rules().seconds(segment.point))))
    }

    pub(crate) fn finish_native_game_segment(
        &mut self,
        segment: Segment,
        work: &mut usize,
    ) -> Result<(), Error> {
        if self.timer_game_time()
            != Some(
                self.native_game_clock
                    .as_ref()
                    .expect("admitted source clock")
                    .rules()
                    .seconds(segment.point),
            )
        {
            return Err(self.inner.error(
                "native game clock",
                Failure::GameClock("source drain did not reach its segment".into()),
            ));
        }
        let mut clock = self
            .native_game_clock
            .as_ref()
            .expect("admitted source clock")
            .clone();
        let finished = if clock.frame_active() {
            clock.finish_frame_segment(Domain::Game, segment)
        } else {
            clock.finish_segment(segment)
        };
        finished.map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))?;
        if segment.ends_cycle {
            let timers = self.prepare_native_timer_rebase(work)?;
            let physics = self.prepare_native_physics_rebase(work)?;
            *work = work.checked_sub(self.autocast_ticks.len()).ok_or_else(|| {
                self.inner
                    .error("native game clock", Failure::Budget("autocast rebase"))
            })?;
            let mut auto = Vec::new();
            auto.try_reserve(self.autocast_ticks.len())
                .map_err(|_| self.inner.error("native game clock", Failure::Capacity))?;
            for &(unit, alias, legacy, point) in &self.autocast_ticks {
                let point = point.ok_or_else(|| {
                    self.inner.error(
                        "native game clock",
                        Failure::GameClock("source autocast request missing".into()),
                    )
                })?;
                if point.cycle != segment.point.cycle {
                    return Err(self.inner.error(
                        "native game clock",
                        Failure::GameClock("autocast request cycle differs".into()),
                    ));
                }
                auto.push((
                    unit,
                    alias,
                    legacy,
                    Some(clock.rules().rebase(point).map_err(|e| {
                        self.inner.error("native game clock", Failure::GameClock(e))
                    })?),
                ));
            }
            self.commit_native_timer_rebase(timers);
            self.commit_native_physics_admission(physics);
            self.autocast_ticks = auto;
        }
        self.native_game_clock = Some(clock);
        Ok(())
    }
}

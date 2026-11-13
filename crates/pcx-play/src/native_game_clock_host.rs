//! The published source clock stays with its canonical arena and timer queue.
use super::{Error, Failure, ObjectHost};
use crate::native_game_clock::{Driver, Point, Segment, State};
use std::time::Duration;

impl ObjectHost {
    pub(crate) fn admit_native_game_clock(&mut self) -> Result<(), Error> {
        let Some(rules) = self.numeric.as_ref().and_then(|p| p.native_game_clock()) else {
            return Ok(());
        };
        if self.native_game_clock.is_some()
            || self.timer_game_time() != Some(f64::from(rules.simple.origin))
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
        self.native_game_clock = Some(
            State::new(rules)
                .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))?,
        );
        Ok(())
    }

    pub fn native_game_point(&self) -> Option<Point> {
        self.native_game_clock
            .as_ref()
            .filter(|clock| clock.driver() != Driver::Legacy)
            .map(State::point)
    }

    pub(crate) fn select_legacy_clock_domain(
        &mut self,
        until: f64,
        advances_clock: bool,
    ) -> Result<(), Error> {
        self.validate_timer_target(until)?;
        let advances = advances_clock
            && (Some(until) != self.timer_game_time()
                || self.next_timer_deadline().is_some_and(|at| at <= until));
        if let Some(clock) = &mut self.native_game_clock {
            clock
                .select_legacy(advances)
                .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))?;
        }
        Ok(())
    }
    pub(crate) fn begin_native_queue_callback(&mut self, point: Point) -> Result<(), Error> {
        if !self.native_clock_driving() {
            return Err(self.inner.error(
                "native game clock",
                Failure::GameClock("native producer is not active".into()),
            ));
        }
        self.native_game_clock
            .as_mut()
            .expect("native clock")
            .enter_callback(point)
            .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))
    }
    pub(crate) fn end_native_queue_callback(&mut self) -> Result<(), Error> {
        self.native_game_clock
            .as_mut()
            .expect("native clock")
            .leave_callback()
            .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))
    }
    pub(super) fn native_clock_driving(&self) -> bool {
        self.native_game_clock
            .as_ref()
            .is_some_and(|clock| clock.driver() == Driver::Native)
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
        let segment = clock
            .next_segment(work)
            .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))?;
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
        clock
            .finish_segment(segment)
            .map_err(|e| self.inner.error("native game clock", Failure::GameClock(e)))?;
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

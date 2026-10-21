//! Running time of day on the owned game clock, rated by the selected `[Misc]` profile.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_objects::editor::misc::DayCycle;
use pcx_script::{Value, ast::Expr, ast::Literal};

const SET: &str = "SetFloatGameState";
const NATIVES: [(&str, &[&str], &str); 4] = [
    (SET, &["fgamestate", "real"], "nothing"),
    ("SuspendTimeOfDay", &["boolean"], "nothing"),
    ("SetTimeOfDayScale", &["real"], "nothing"),
    ("GetTimeOfDayScale", &[], "real"),
];

/// `hour` holds at game time `since`; later hours project linearly from it.
pub(super) struct TimeOfDay {
    hours_per_second: f64,
    day_hours: f64,
    hour: f64,
    since: f64,
    scale: f64,
    suspended: bool,
    /// `[dawn, dusk)` in hours from the stock `bj_TOD_DAWN`/`bj_TOD_DUSK` globals, when the script declares them.
    day_window: Option<[f64; 2]>,
}

impl TimeOfDay {
    fn hour_at(&self, now: f64) -> f64 {
        let elapsed = now - self.since;
        if self.suspended || elapsed <= 0.0 {
            return self.hour;
        }
        (self.hour + elapsed * self.scale * self.hours_per_second).rem_euclid(self.day_hours)
    }
    fn rebase(&mut self, now: f64) {
        self.hour = self.hour_at(now);
        self.since = now;
    }
    /// Seconds in `[from, to)` whose hour lies in the day window. The hour is linear
    /// between rebases, so the window edges are crossed at exact times, and each
    /// piece between crossings is classified by its midpoint.
    fn daytime_between(&self, from: f64, to: f64, [dawn, dusk]: [f64; 2]) -> f64 {
        if to <= from {
            return 0.0;
        }
        let inside = |hour: f64| hour >= dawn && hour < dusk;
        let start = self.hour_at(from);
        let rate = if self.suspended {
            0.0
        } else {
            self.scale * self.hours_per_second
        };
        if rate == 0.0 {
            return if inside(start) { to - from } else { 0.0 };
        }
        let end = start + rate * (to - from);
        let (low, high) = (start.min(end), start.max(end));
        let mut cuts = vec![from, to];
        for edge in [dawn, dusk] {
            let first = ((low - edge) / self.day_hours).floor() as i64;
            let last = ((high - edge) / self.day_hours).ceil() as i64;
            for cycle in first..=last {
                let hour = edge + cycle as f64 * self.day_hours;
                if hour > low && hour < high {
                    cuts.push(from + (hour - start) / rate);
                }
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.windows(2)
            .filter(|w| w[1] > w[0])
            .map(|w| {
                let mid = (w[0] + w[1]) / 2.0;
                let hour = (start + rate * (mid - from)).rem_euclid(self.day_hours);
                if inside(hour) { w[1] - w[0] } else { 0.0 }
            })
            .sum()
    }
}

impl ObjectHost {
    /// Starts from the startup clock's hour and scale. The cycle runs until
    /// `SuspendTimeOfDay(true)`: an unsuspended start is a recreation choice.
    pub fn admit_time_of_day(&mut self, cycle: &DayCycle) -> Result<(), Error> {
        let fail = |reason| self.inner.error(SET, Failure::ClockState(reason));
        let (Some(clock), Some(now)) = (&self.startup_clock, self.timer_game_time()) else {
            return Err(fail("startup clock and game time required"));
        };
        if self.time_of_day.is_some() {
            return Err(fail("already admitted"));
        }
        let source = self
            .inner
            .program
            .natives()
            .get("GetFloatGameState")
            .map(|n| n.source.clone())
            .ok_or_else(|| fail("GetFloatGameState declaration required"))?;
        for (name, args, result) in NATIVES {
            Self::contract(&self.inner, name, args, result, false, None)?;
            if self.inner.program.natives().get(name).map(|n| &n.source) != Some(&source) {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        let day_window = self
            .stock_real_constant("bj_TOD_DAWN")
            .zip(self.stock_real_constant("bj_TOD_DUSK"))
            .map(|(dawn, dusk)| [dawn, dusk]);
        let state = TimeOfDay {
            hours_per_second: f64::from(cycle.day_hours) / f64::from(cycle.day_length),
            day_hours: f64::from(cycle.day_hours),
            hour: f64::from(clock.input.hour()),
            since: now,
            scale: f64::from(clock.input.scale()),
            suspended: false,
            day_window,
        };
        self.time_of_day = Some(state);
        Ok(())
    }

    /// A real literal initializer of a selected constant global, e.g. `bj_TOD_DAWN`.
    fn stock_real_constant(&self, name: &str) -> Option<f64> {
        let global = self
            .inner
            .program
            .globals()
            .iter()
            .find(|g| g.name == name && g.is_constant && !g.array)?;
        match &global.init {
            Some(Expr::Literal(Literal::Real(real), _)) => Some(real.value),
            _ => None,
        }
    }

    /// Seconds of game time in `[from, to)` inside the stock day window. `Ok(None)` when the
    /// clock or the script's dawn/dusk constants are not admitted. A span starting before the
    /// last rebase cannot be classified from the stored hour, so it is refused.
    pub(super) fn daytime_between(&self, from: f64, to: f64) -> Result<Option<f64>, String> {
        let Some(state) = self.time_of_day.as_ref() else {
            return Ok(None);
        };
        if from < state.since {
            return Err("span predates the time-of-day rebase".into());
        }
        Ok(state
            .day_window
            .map(|window| state.daytime_between(from, to, window)))
    }

    /// Current fraction of the day in 0..1, or None before admission.
    pub fn day_fraction(&self) -> Option<f32> {
        let state = self.time_of_day.as_ref()?;
        let hour = state.hour_at(self.timer_game_time()?);
        Some((hour / state.day_hours).rem_euclid(1.0) as f32)
    }

    /// Current hour, or None before admission.
    pub fn time_of_day(&self) -> Option<f32> {
        let now = self.timer_game_time()?;
        Some(self.time_of_day.as_ref()?.hour_at(now) as f32)
    }

    pub(super) fn natural_sleep_night(&self) -> Option<bool> {
        let state = self.time_of_day.as_ref()?;
        let [dawn, dusk] = state.day_window?;
        let hour = state.hour_at(self.timer_game_time()?);
        Some(hour < dawn || hour >= dusk)
    }

    pub(super) fn time_of_day_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.time_of_day.as_ref()?;
        if !NATIVES.iter().any(|&(n, ..)| n == name) {
            return None;
        }
        Some((|| {
            let fail = |reason| self.inner.error(name, Failure::ClockState(reason));
            let now = self
                .timer_game_time()
                .ok_or_else(|| fail("game time missing"))?;
            let finite = |v: &Value| {
                v.as_real()
                    .filter(|r| r.is_finite())
                    .map(f64::from)
                    .ok_or_else(|| fail("finite real required"))
            };
            let selector = self.startup_clock.as_ref().map(|c| c.selector);
            let value = match (name, args) {
                (SET, [Value::Handle(id), value]) => {
                    match self.inner.entry(*id)? {
                        Entry::Symbolic { kind, enumerand }
                            if kind == "fgamestate" && Some(*enumerand) == selector => {}
                        Entry::Symbolic { kind, .. } if kind == "fgamestate" => {
                            return Err(fail("unsupported game state"));
                        }
                        _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                    }
                    let hour = finite(value)?;
                    self.advance_unit_regen(now)?;
                    let state = self.time_of_day.as_mut().expect("admitted");
                    state.hour = hour;
                    state.since = now;
                    Value::Null
                }
                ("SuspendTimeOfDay", [Value::Bool(suspended)]) => {
                    self.advance_unit_regen(now)?;
                    let state = self.time_of_day.as_mut().expect("admitted");
                    state.rebase(now);
                    state.suspended = *suspended;
                    Value::Null
                }
                ("SetTimeOfDayScale", [value]) => {
                    let scale = finite(value)?;
                    self.advance_unit_regen(now)?;
                    let state = self.time_of_day.as_mut().expect("admitted");
                    state.rebase(now);
                    state.scale = scale;
                    Value::Null
                }
                ("GetTimeOfDayScale", []) => {
                    Value::Real(self.time_of_day.as_ref().expect("admitted").scale as f32)
                }
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            Ok(value)
        })())
    }
}

#[cfg(test)]
mod tests {
    use super::TimeOfDay;

    /// A 480 s day of 24 hours, with the stock 6:00 to 18:00 window.
    fn clock(hour: f64, scale: f64, suspended: bool) -> TimeOfDay {
        TimeOfDay {
            hours_per_second: 24.0 / 480.0,
            day_hours: 24.0,
            hour,
            since: 0.0,
            scale,
            suspended,
            day_window: Some([6.0, 18.0]),
        }
    }

    #[test]
    fn daytime_integrates_exact_window_crossings_and_ignores_chunking() {
        let forward = clock(5.0, 1.0, false);
        // 5:00 to 29:00 crosses dawn and dusk once each: twelve hours at 0.05 h/s.
        assert!((forward.daytime_between(0.0, 480.0, [6.0, 18.0]) - 240.0).abs() < 1e-9);
        let chunked: f64 = (0..480)
            .step_by(7)
            .map(|s| {
                let (from, to) = (f64::from(s), f64::from((s + 7).min(480)));
                forward.daytime_between(from, to, [6.0, 18.0])
            })
            .sum();
        assert!((chunked - 240.0).abs() < 1e-9, "{chunked}");
        // Negative scale runs backwards: 7:00 to 6:00 is day, the night wraps, then 18:00 to 7:00.
        let backward = clock(7.0, -1.0, false);
        assert!((backward.daytime_between(0.0, 480.0, [6.0, 18.0]) - 240.0).abs() < 1e-9);
    }

    #[test]
    fn suspended_or_stopped_clock_holds_its_hour() {
        assert_eq!(
            clock(5.0, 1.0, true).daytime_between(0.0, 480.0, [6.0, 18.0]),
            0.0
        );
        assert_eq!(
            clock(12.0, 1.0, true).daytime_between(0.0, 480.0, [6.0, 18.0]),
            480.0
        );
        assert_eq!(
            clock(12.0, 0.0, false).daytime_between(0.0, 480.0, [6.0, 18.0]),
            480.0
        );
        assert_eq!(
            clock(5.0, 0.0, false).daytime_between(0.0, 480.0, [6.0, 18.0]),
            0.0
        );
        assert_eq!(
            clock(12.0, 1.0, false).daytime_between(5.0, 5.0, [6.0, 18.0]),
            0.0
        );
    }

    #[test]
    fn spans_split_at_each_rebase_compose_to_the_scripted_clock() {
        // Rate +0.05 h/s from 5:00 to 10:00 at t=100 (80 s of day), then a scale change
        // to -1 at t=100 from 10:00: 10:00 to 6:00 is day (80 s), and 18:00 to 15:00 at
        // t=420 to t=480 is day (60 s). The second span is 140 s, the total 220 s.
        let before = clock(5.0, 1.0, false);
        let after = TimeOfDay {
            hour: before.hour_at(100.0),
            since: 100.0,
            scale: -1.0,
            ..clock(0.0, 0.0, false)
        };
        let window = [6.0, 18.0];
        assert!((before.daytime_between(0.0, 100.0, window) - 80.0).abs() < 1e-9);
        assert!((after.daytime_between(100.0, 480.0, window) - 140.0).abs() < 1e-9);
        let split = after.daytime_between(100.0, 300.0, window)
            + after.daytime_between(300.0, 480.0, window);
        assert!((split - 140.0).abs() < 1e-9, "{split}");
    }
}

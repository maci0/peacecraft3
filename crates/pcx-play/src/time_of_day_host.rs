//! Running time of day on the owned game clock, rated by the selected `[Misc]` profile.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_objects::editor::misc::DayCycle;
use pcx_script::Value;

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
        let state = TimeOfDay {
            hours_per_second: f64::from(cycle.day_hours) / f64::from(cycle.day_length),
            day_hours: f64::from(cycle.day_hours),
            hour: f64::from(clock.input.hour()),
            since: now,
            scale: f64::from(clock.input.scale()),
            suspended: false,
        };
        self.time_of_day = Some(state);
        Ok(())
    }

    /// Current hour, or None before admission.
    pub fn time_of_day(&self) -> Option<f32> {
        let now = self.timer_game_time()?;
        Some(self.time_of_day.as_ref()?.hour_at(now) as f32)
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
                    let state = self.time_of_day.as_mut().expect("admitted");
                    state.hour = hour;
                    state.since = now;
                    Value::Null
                }
                ("SuspendTimeOfDay", [Value::Bool(suspended)]) => {
                    let state = self.time_of_day.as_mut().expect("admitted");
                    state.rebase(now);
                    state.suspended = *suspended;
                    Value::Null
                }
                ("SetTimeOfDayScale", [value]) => {
                    let scale = finite(value)?;
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

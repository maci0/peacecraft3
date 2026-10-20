//! Executed widget indicator flashes (`AddIndicator`, `UnitAddIndicator`) and
//! minimap pings (`PingMinimap`) for the renderer: target or point, authored
//! color/duration and game time. Flash duration and blending are unqualified;
//! nothing here draws.
use super::{Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Indicator {
    pub widget: HandleId,
    pub rgba: [i32; 4],
    pub at: f64,
}

const NATIVES: [(&str, &str); 2] = [("AddIndicator", "widget"), ("UnitAddIndicator", "unit")];
const PING: &str = "PingMinimap";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinimapPing {
    pub x: f32,
    pub y: f32,
    pub duration: f32,
    pub at: f64,
}

impl ObjectHost {
    pub fn with_indicators(mut self, max_records: usize) -> Result<Self, Error> {
        for (name, target) in NATIVES {
            if self.inner.program.natives().get(name).is_none() {
                continue;
            }
            Self::contract_in(
                &self.inner,
                name,
                &[target, "integer", "integer", "integer", "integer"],
                "nothing",
                false,
                None,
                false,
            )?;
        }
        if self.inner.program.natives().get(PING).is_some() {
            Self::contract_in(
                &self.inner,
                PING,
                &["real", "real", "real"],
                "nothing",
                false,
                None,
                false,
            )?;
        }
        std::alloc::Layout::array::<Indicator>(max_records)
            .and_then(|_| std::alloc::Layout::array::<MinimapPing>(max_records))
            .map_err(|_| self.inner.error(NATIVES[0].0, Failure::Capacity))?;
        self.indicators = Some((Vec::new(), max_records));
        self.pings = Vec::new();
        Ok(self)
    }

    /// Pings in execution order; the oldest are dropped past the bound.
    pub fn minimap_pings(&self) -> &[MinimapPing] {
        &self.pings
    }

    /// Flashes in execution order; the oldest are dropped past the bound.
    pub fn indicators(&self) -> &[Indicator] {
        self.indicators.as_ref().map_or(&[], |(list, _)| list)
    }

    pub(super) fn indicator_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let max = self.indicators.as_ref()?.1;
        if name == PING {
            self.inner.program.natives().get(name)?;
            let reals: Option<Vec<f32>> = args.iter().map(|v| v.as_real()).collect();
            let Some(&[x, y, duration]) = reals.as_deref() else {
                return Some(Err(self.inner.error(name, Failure::Signature)));
            };
            if self.pings.len() >= max {
                self.pings.remove(0);
            }
            let at = self.timer_game_time().unwrap_or(0.0);
            self.pings.push(MinimapPing { x, y, duration, at });
            return Some(Ok(Value::Null));
        }
        if !NATIVES.iter().any(|&(n, _)| n == name) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some((|| {
            let widget = match args.first() {
                Some(Value::Null) => return Ok(Value::Null),
                Some(Value::Handle(id)) => *id,
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            let mut rgba = [0; 4];
            for (slot, value) in rgba.iter_mut().zip(&args[1..]) {
                let Value::Int(v) = value else {
                    return Err(self.inner.error(name, Failure::Signature));
                };
                *slot = *v;
            }
            let at = self.timer_game_time().unwrap_or(0.0);
            let (list, max) = self.indicators.as_mut().expect("admitted");
            if list.len() >= *max {
                list.remove(0);
            }
            list.push(Indicator { widget, rgba, at });
            Ok(Value::Null)
        })())
    }
}

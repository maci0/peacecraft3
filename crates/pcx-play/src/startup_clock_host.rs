use super::{Entry, Error, Failure, ObjectHost};
use crate::playercolor_host::integer;
use crate::startup_clock::StartupClock;
use pcx_script::Value;
use pcx_script::ast::Expr;

/// The selected source constant and immutable pre-simulation clock input.
#[derive(Debug)]
pub struct StartupClockState {
    pub input: StartupClock,
    pub selector: i32,
}
impl ObjectHost {
    /// Admits only a source-defined fgamestate constant. `max_globals` bounds
    /// selection work; expression limits use the same closed evaluator as enums.
    pub fn with_startup_clock(
        mut self,
        input: StartupClock,
        global_name: &str,
        max_globals: usize,
        expression_depth: usize,
        expression_work: usize,
    ) -> Result<Self, Error> {
        let name = "GetFloatGameState";
        let fail = |reason| self.inner.error(name, Failure::ClockState(reason));
        if self.startup_clock.is_some() {
            return Err(fail("already admitted"));
        }
        Self::contract(&self.inner, name, &["fgamestate"], "real", true, None)?;
        if self.inner.program.globals().len() > max_globals {
            return Err(self
                .inner
                .error(name, Failure::Budget("clock global selection")));
        }
        let global = self
            .inner
            .program
            .globals()
            .iter()
            .find(|g| g.name == global_name)
            .ok_or_else(|| fail("missing source constant"))?;
        if !global.is_constant
            || global.array
            || global.ty.array
            || global.ty.kind.name() != "fgamestate"
        {
            return Err(fail("expected constant scalar fgamestate"));
        }
        let Some(Expr::Call { callee, args, .. }) = &global.init else {
            return Err(fail("expected admitted conversion initializer"));
        };
        let (Expr::Ident(conversion, _), [arg]) = (callee.as_ref(), args.as_slice()) else {
            return Err(fail("expected one conversion argument"));
        };
        let contract = self
            .inner
            .conversions
            .get(conversion.as_str())
            .filter(|c| c.kind == "fgamestate" && c.source == global.source)
            .ok_or_else(|| fail("conversion/source is not admitted"))?;
        let mut work = expression_work;
        let selector =
            integer(arg, expression_depth, &mut work).map_err(|e| self.inner.error(name, e))?;
        if !contract.domain.contains(&selector) {
            return Err(fail("selector outside admitted domain"));
        }
        self.startup_clock = Some(StartupClockState { input, selector });
        Ok(self)
    }
    pub fn startup_clock_state(&self) -> Option<&StartupClockState> {
        self.startup_clock.as_ref()
    }
    pub(super) fn startup_clock_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let state = self.startup_clock.as_ref()?;
        if name != "GetFloatGameState" {
            return None;
        }
        Some((|| {
            let [Value::Handle(id)] = args else {
                return Err(self
                    .inner
                    .error(name, Failure::ClockState("expected fgamestate identity")));
            };
            match self.inner.entry(*id)? {
                Entry::Symbolic { kind, enumerand }
                    if kind == "fgamestate" && *enumerand == state.selector =>
                {
                    Ok(Value::Real(
                        self.time_of_day().unwrap_or_else(|| state.input.hour()),
                    ))
                }
                Entry::Symbolic { kind, .. } if kind == "fgamestate" => Err(self
                    .inner
                    .error(name, Failure::ClockState("unsupported game state"))),
                _ => Err(self.inner.error(name, Failure::WrongFamily)),
            }
        })())
    }
}

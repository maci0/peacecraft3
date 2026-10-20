use super::{Error, Failure, ObjectHost};
use crate::script_random::{
    profile::Policy,
    state::{self, State},
};
use pcx_script::Value;
use std::sync::Arc;

pub(super) struct Random {
    policy: Arc<Policy>,
    max_secondary: usize,
    state: Option<State>,
}

impl ObjectHost {
    /// Parameters and an allocation ceiling admit the three declarations. State
    /// begins only at an executed `SetRandomSeed`; map/session replacement drops it.
    pub fn with_script_random(
        mut self,
        policy: Arc<Policy>,
        max_secondary: usize,
    ) -> Result<Self, Error> {
        let fail = |reason| {
            self.inner
                .error("SetRandomSeed", Failure::RandomSetup(reason))
        };
        if self.random.is_some() {
            return Err(fail("already admitted"));
        }
        if policy.secondary_count > max_secondary {
            return Err(fail("secondary state exceeds allocation ceiling"));
        }
        let mut source = None;
        for (name, parameters, result) in [
            ("GetRandomInt", &["integer", "integer"][..], "integer"),
            ("GetRandomReal", &["real", "real"][..], "real"),
            ("SetRandomSeed", &["integer"][..], "nothing"),
        ] {
            Self::contract(&self.inner, name, parameters, result, false, None)?;
            let declaration = self.inner.program.natives().get(name).unwrap();
            if source
                .replace(&declaration.source)
                .is_some_and(|old| old != &declaration.source)
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.random = Some(Random {
            policy,
            max_secondary,
            state: None,
        });
        Ok(self)
    }

    /// One draw from the shared script RNG, as `GetRandomInt` makes.
    pub(super) fn draw_random_integer(
        &mut self,
        low: i32,
        high: i32,
        caller: &str,
    ) -> Result<i32, Error> {
        let random = self.random.as_mut().ok_or_else(|| {
            self.inner
                .error(caller, Failure::RandomSetup("not admitted"))
        })?;
        state::integer(&random.policy, &mut random.state, low, high)
            .map_err(|error| self.inner.error(caller, Failure::RandomState(error)))
    }

    pub fn script_random_state(&self) -> Option<&State> {
        self.random.as_ref()?.state.as_ref()
    }

    pub(super) fn random_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !matches!(name, "GetRandomInt" | "GetRandomReal" | "SetRandomSeed") {
            return None;
        }
        let random = self.random.as_mut()?;
        let result = match (name, args) {
            ("SetRandomSeed", [Value::Int(seed)]) => {
                state::seeded(&random.policy, *seed, random.max_secondary).map(|next| {
                    random.state = Some(next);
                    Value::Null
                })
            }
            ("GetRandomInt", [Value::Int(low), Value::Int(high)]) => {
                state::integer(&random.policy, &mut random.state, *low, *high).map(Value::Int)
            }
            ("GetRandomReal", [Value::Real(low), Value::Real(high)]) => {
                state::real(&random.policy, &mut random.state, *low, *high).map(Value::Real)
            }
            _ => {
                return Some(Err(self.inner.error(
                    name,
                    Failure::RandomSetup("arguments do not match declaration"),
                )));
            }
        };
        Some(result.map_err(|error| self.inner.error(name, Failure::RandomState(error))))
    }
}

#[cfg(test)]
#[path = "script_random_host_tests.rs"]
mod tests;

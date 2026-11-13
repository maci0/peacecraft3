use super::{Error, Failure, ObjectHost};
use crate::script_random::{
    order::{CallSite, DrawLog, DrawRecord},
    profile::Policy,
    state::{self, PrimaryCheckpoint, State},
};
use pcx_script::Value;
use std::sync::Arc;

#[path = "script_random_checkpoint.rs"]
mod checkpoint;

pub(super) struct Random {
    policy: Arc<Policy>,
    max_secondary: usize,
    state: Option<State>,
    /// Caller sequence in execution order. Reseeds clear it with the state;
    /// refused calls record nothing.
    order: DrawLog,
}

impl ObjectHost {
    pub(super) fn random_policy(&self) -> Option<Arc<Policy>> {
        self.random.as_ref().map(|r| Arc::clone(&r.policy))
    }
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
            order: DrawLog::new(),
        });
        Ok(self)
    }

    /// One draw from the shared script RNG, as `GetRandomInt` makes.
    /// Admitted calls are appended to the caller sequence in execution order.
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
        random
            .order
            .reserve()
            .map_err(|error| self.inner.error(caller, Failure::RandomState(error)))?;
        let site = CallSite::gameplay(caller)
            .map_err(|error| self.inner.error(caller, Failure::RandomState(error)))?;
        let value = state::integer(&random.policy, &mut random.state, low, high)
            .map_err(|error| self.inner.error(caller, Failure::RandomState(error)))?;
        let primary_word = random.state.as_ref().map(|state| state.primary_word());
        random.order.push(DrawRecord {
            site,
            primary_word,
            secondary_word: None,
        });
        Ok(value)
    }

    /// One source-qualified gameplay die from an already retained secondary stream.
    /// The ability policy supplies the channel and sides; this does not seed or
    /// borrow the primary script stream. Admitted calls join the caller sequence.
    pub fn draw_random_secondary_die(
        &mut self,
        channel: usize,
        sides: u32,
        caller: &str,
    ) -> Result<u32, Error> {
        let random = self.random.as_mut().ok_or_else(|| {
            self.inner
                .error(caller, Failure::RandomSetup("not admitted"))
        })?;
        random
            .order
            .reserve()
            .map_err(|error| self.inner.error(caller, Failure::RandomState(error)))?;
        let value = state::secondary_die(&random.policy, &mut random.state, channel, sides)
            .map_err(|error| self.inner.error(caller, Failure::RandomState(error)))?;
        let primary_word = random.state.as_ref().map(|state| state.primary_word());
        random.order.push(DrawRecord {
            site: CallSite::SecondaryDie { channel },
            primary_word,
            secondary_word: random
                .state
                .as_ref()
                .and_then(|state| state.secondary_words().nth(channel)),
        });
        Ok(value)
    }

    /// One real in `[0, 1)` from a retained secondary stream, as the gameplay draw at `0x69a030`
    /// makes. It does not touch the primary stream and joins the caller sequence.
    pub(super) fn draw_random_secondary_real(
        &mut self,
        channel: usize,
        caller: &str,
    ) -> Result<f32, Error> {
        let random = self.random.as_mut().ok_or_else(|| {
            self.inner
                .error(caller, Failure::RandomSetup("not admitted"))
        })?;
        random
            .order
            .reserve()
            .map_err(|error| self.inner.error(caller, Failure::RandomState(error)))?;
        let value = state::secondary_real(&random.policy, &mut random.state, channel)
            .map_err(|error| self.inner.error(caller, Failure::RandomState(error)))?;
        let primary_word = random.state.as_ref().map(|state| state.primary_word());
        random.order.push(DrawRecord {
            site: CallSite::SecondaryReal { channel },
            primary_word,
            secondary_word: random
                .state
                .as_ref()
                .and_then(|state| state.secondary_words().nth(channel)),
        });
        Ok(value)
    }

    pub fn script_random_state(&self) -> Option<&State> {
        self.random.as_ref()?.state.as_ref()
    }

    /// Caller sequence in execution order, oldest first. Reseeds clear it with
    /// the generator state; refused calls record nothing.
    pub fn script_random_calls(&self) -> Option<&[DrawRecord]> {
        Some(self.random.as_ref()?.order.records())
    }

    /// Save the primary word and lanes plus the caller-sequence length, so a
    /// refused combat hit can restore the draw that its damage roll consumed
    /// without copying secondary history. Restoring also drops later caller
    /// records, keeping the sequence aligned with the generator.
    pub(super) fn script_random_primary_checkpoint(&self) -> Option<PrimaryCheckpoint> {
        let random = self.random.as_ref()?;
        let state = random.state.as_ref()?;
        Some(state.primary_checkpoint(random.order.records().len()))
    }

    pub(super) fn restore_script_random_primary(&mut self, checkpoint: Option<PrimaryCheckpoint>) {
        let Some(checkpoint) = checkpoint else {
            return;
        };
        let order_len = State::checkpoint_order_len(&checkpoint);
        if let Some(random) = self.random.as_mut() {
            if let Some(state) = random.state.as_mut() {
                state.restore_primary(checkpoint);
            }
            random.order.truncate(order_len);
        }
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
                    random.order.clear();
                    Value::Null
                })
            }
            ("GetRandomInt", [Value::Int(low), Value::Int(high)]) => random
                .order
                .reserve()
                .and_then(|()| state::integer(&random.policy, &mut random.state, *low, *high))
                .map(|value| {
                    let primary_word = random.state.as_ref().map(|state| state.primary_word());
                    random.order.push(DrawRecord {
                        site: CallSite::ScriptInteger,
                        primary_word,
                        secondary_word: None,
                    });
                    Value::Int(value)
                }),
            ("GetRandomReal", [Value::Real(low), Value::Real(high)]) => random
                .order
                .reserve()
                .and_then(|()| state::real(&random.policy, &mut random.state, *low, *high))
                .map(|value| {
                    let primary_word = random.state.as_ref().map(|state| state.primary_word());
                    random.order.push(DrawRecord {
                        site: CallSite::ScriptReal,
                        primary_word,
                        secondary_word: None,
                    });
                    Value::Real(value)
                }),
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

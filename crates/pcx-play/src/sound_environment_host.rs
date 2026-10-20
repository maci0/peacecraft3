//! `NewSoundEnvironment` selection. The call selects one admitted environment record by
//! exact name and computes the values the native applies from it. Delivery to a 3D
//! provider is not modelled here; the audio consumer owns that state.
use super::{Error, Failure, ObjectHost};
use crate::sound_environment::{Applied, EaxProperty, SoundEnvironment, apply};
use pcx_script::Value;

const NATIVE: &str = "NewSoundEnvironment";

pub(super) struct State {
    environments: Vec<SoundEnvironment>,
    properties: Vec<EaxProperty>,
    current: Option<(usize, Applied)>,
}

impl ObjectHost {
    /// Admits the native for these records and their executable property rows. Records
    /// come from the selected sound tables; an empty list is refused, not admitted.
    pub fn with_sound_environments(
        mut self,
        environments: Vec<SoundEnvironment>,
        properties: Vec<EaxProperty>,
    ) -> Result<Self, Error> {
        if self.sound_environments.is_some() {
            return Err(self.inner.error(
                NATIVE,
                Failure::SoundState("sound environments already admitted"),
            ));
        }
        if environments.is_empty() {
            return Err(self
                .inner
                .error(NATIVE, Failure::SoundState("no sound environment records")));
        }
        Self::contract(&self.inner, NATIVE, &["string"], "nothing", false, None)?;
        self.sound_environments = Some(State {
            environments,
            properties,
            current: None,
        });
        Ok(self)
    }

    /// The last executed `NewSoundEnvironment` record. `None` until a call succeeds.
    pub fn sound_environment(&self) -> Option<&SoundEnvironment> {
        let state = self.sound_environments.as_ref()?;
        let (index, _) = state.current.as_ref()?;
        state.environments.get(*index)
    }

    /// Clamped values of the last executed record: the effect volume and the provider
    /// writes the native makes when a 3D provider is open.
    pub fn sound_environment_applied(&self) -> Option<&Applied> {
        let state = self.sound_environments.as_ref()?;
        state.current.as_ref().map(|(_, applied)| applied)
    }

    pub(super) fn sound_environment_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != NATIVE {
            return None;
        }
        let state = self.sound_environments.as_mut()?;
        let fail = |reason| self.inner.error(name, reason);
        Some((|| {
            let [Value::Str(text)] = args else {
                return Err(fail(Failure::SoundState("expected one environment name")));
            };
            let index = state
                .environments
                .iter()
                .position(|environment| environment.name == *text)
                .ok_or_else(|| fail(Failure::SoundState("unknown sound environment")))?;
            let applied = apply(&state.environments[index], &state.properties)
                .map_err(|_| fail(Failure::SoundState("sound environment values do not apply")))?;
            state.current = Some((index, applied));
            Ok(Value::Null)
        })())
    }
}

#[cfg(test)]
#[path = "sound_environment_host_tests.rs"]
mod tests;

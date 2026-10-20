//! `NewSoundEnvironment` selection. The call selects one admitted environment
//! record by exact name; reverb application belongs to the audio consumer.
use super::{Error, Failure, ObjectHost};
use crate::sound_environment::SoundEnvironment;
use pcx_script::Value;

const NATIVE: &str = "NewSoundEnvironment";

pub(super) struct State {
    environments: Vec<SoundEnvironment>,
    current: Option<usize>,
}

impl ObjectHost {
    /// Admits the native for these records. Records come from the selected sound
    /// tables; an empty list is refused, not admitted as an empty selection.
    pub fn with_sound_environments(
        mut self,
        environments: Vec<SoundEnvironment>,
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
            current: None,
        });
        Ok(self)
    }

    /// The last executed `NewSoundEnvironment` record. `None` until a call succeeds.
    pub fn sound_environment(&self) -> Option<&SoundEnvironment> {
        let state = self.sound_environments.as_ref()?;
        state.environments.get(state.current?)
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
            state.current = Some(index);
            Ok(Value::Null)
        })())
    }
}

#[cfg(test)]
#[path = "sound_environment_host_tests.rs"]
mod tests;

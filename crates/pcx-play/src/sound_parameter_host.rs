use super::{Entry, Error, Failure, ObjectHost};
use crate::sound_parameters::{Bank, Parameters};
use pcx_script::{HandleId, Value};

impl ObjectHost {
    pub fn with_sound_parameters(mut self, bank: Bank) -> Result<Self, Error> {
        let name = "SetSoundParamsFromLabel";
        if self.sound_parameters.is_some() || (self.sounds.is_none() && self.file_sounds.is_none())
        {
            return Err(self.inner.error(
                name,
                Failure::SoundState("sound resources required; parameters must be admitted once"),
            ));
        }
        Self::contract(
            &self.inner,
            name,
            &["sound", "string"],
            "nothing",
            false,
            Some("sound"),
        )?;
        self.sound_parameters = Some(bank);
        Ok(self)
    }
    pub fn sound_parameter_bank(&self) -> Option<&Bank> {
        self.sound_parameters.as_ref()
    }
    /// Last executed label assignment. The resource and original creation inputs
    /// remain intact; raw table flags are not claimed as effective native flags.
    pub fn sound_parameters(&self, id: HandleId) -> Result<Option<Parameters<'_>>, Error> {
        let name = "SetSoundParamsFromLabel";
        self.sound_resource(id)
            .map_err(|e| self.inner.error(name, e.failure))?;
        let (Entry::Sound { parameters, .. } | Entry::FileSound { parameters, .. }) =
            self.inner.entry(id)?
        else {
            return Err(self.inner.error(name, Failure::WrongFamily));
        };
        parameters
            .map(|index| {
                self.sound_parameters
                    .as_ref()
                    .and_then(|b| b.get(index))
                    .ok_or_else(|| self.inner.error(name, Failure::Identity))
            })
            .transpose()
    }
    pub(super) fn sound_parameter_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != "SetSoundParamsFromLabel" {
            return None;
        }
        self.sound_parameters.as_ref()?;
        Some((|| {
            let fail = |reason| self.inner.error(name, reason);
            let [Value::Handle(id), Value::Str(label)] = args else {
                return Err(fail(Failure::SoundState(
                    "expected sound identity and label",
                )));
            };
            self.sound_resource(*id).map_err(|e| fail(e.failure))?;
            let bank = self
                .sound_parameters
                .as_ref()
                .ok_or_else(|| fail(Failure::SoundState("parameters not admitted")))?;
            if label.len() > bank.label_limit() {
                return Err(fail(Failure::Budget("sound parameter label")));
            }
            let index = bank
                .find(label)
                .ok_or_else(|| fail(Failure::MissingSoundLabel(label.clone())))?;
            let slot = usize::try_from(id.token() - 1).map_err(|_| fail(Failure::Identity))?;
            let (Entry::Sound { parameters, .. } | Entry::FileSound { parameters, .. }) =
                &mut self.inner.arena[slot]
            else {
                return Err(self.inner.error(name, Failure::WrongFamily));
            };
            *parameters = Some(index);
            Ok(Value::Null)
        })())
    }
}

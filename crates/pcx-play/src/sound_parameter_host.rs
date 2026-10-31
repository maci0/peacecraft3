use super::{Entry, Error, Failure, ObjectHost};
use crate::sound_parameters::{Assignment, Bank, Banks, ParameterValues, Parameters};
use pcx_script::{HandleId, Value};
pub(super) type Loader = Box<dyn FnMut(&str) -> Result<Banks, String> + Send + Sync>;

impl ObjectHost {
    pub fn admit_sound_parameter_loader(
        &mut self,
        budget: crate::sound_parameters::BanksBudget,
        loader: impl FnMut(&str) -> Result<Banks, String> + Send + Sync + 'static,
    ) -> Result<(), Error> {
        let fail = |problem| self.inner.error("SetSoundParamsFromLabel", problem);
        if self.sound_parameter_loader.is_some() {
            return Err(fail(Failure::SoundState(
                "parameter loader already admitted",
            )));
        }
        let banks = self
            .sound_parameters
            .as_mut()
            .ok_or_else(|| fail(Failure::SoundState("parameter banks required")))?;
        banks
            .admit_budget(budget)
            .map_err(|e| fail(Failure::SoundSource(e.to_string())))?;
        self.sound_parameter_loader = Some((Box::new(loader), budget));
        Ok(())
    }
    pub fn with_sound_parameters(self, bank: Bank) -> Result<Self, Error> {
        self.with_sound_parameter_banks(Banks::single(bank))
    }
    /// Install a completely preflighted, source-qualified collection once.
    pub fn with_sound_parameter_banks(mut self, banks: Banks) -> Result<Self, Error> {
        let name = "SetSoundParamsFromLabel";
        if self.sound_parameters.is_some() || !self.has_sound_resources() {
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
        self.sound_parameters = Some(banks);
        Ok(self)
    }
    pub fn sound_parameter_bank(&self) -> Option<&Bank> {
        self.sound_parameters.as_ref()?.single_full()
    }
    pub fn sound_parameter_banks(&self) -> Option<&Banks> {
        self.sound_parameters.as_ref()
    }
    /// Last executed label assignment. The resource and original creation inputs
    /// remain intact; raw table flags are not claimed as effective native flags.
    pub fn sound_parameters(&self, id: HandleId) -> Result<Option<Parameters<'_>>, Error> {
        self.sound_parameter_assignment(id)?
            .map(|assignment| match assignment.parameters {
                ParameterValues::Full(parameters) => Ok(parameters),
                ParameterValues::Dialogue(_) => Err(self.inner.error(
                    "SetSoundParamsFromLabel",
                    Failure::UnsupportedSoundParameterProfile(assignment.source.profile),
                )),
            })
            .transpose()
    }
    /// Last assignment with its actual source table, row, receipt and schema.
    pub fn sound_parameter_assignment(
        &self,
        id: HandleId,
    ) -> Result<Option<Assignment<'_>>, Error> {
        let name = "SetSoundParamsFromLabel";
        self.sound_admitted(id)
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
            let [Value::Handle(id), Value::Str(label)] = args else {
                return Err(self.inner.error(
                    name,
                    Failure::SoundState("expected sound identity and label"),
                ));
            };
            let id = *id;
            let label = label.clone();
            self.sound_admitted(id)
                .map_err(|e| self.inner.error(name, e.failure))?;
            let index = self.prepare_sound_parameter_label(name, &label)?;
            let slot = usize::try_from(id.token() - 1)
                .map_err(|_| self.inner.error(name, Failure::Identity))?;
            let (Entry::Sound { parameters, .. } | Entry::FileSound { parameters, .. }) =
                &mut self.inner.arena[slot]
            else {
                return Err(self.inner.error(name, Failure::WrongFamily));
            };
            *parameters = Some(index);
            Ok(Value::Null)
        })())
    }

    /// Resolve one admitted label, appending through the loader when it is absent.
    /// The returned key is the stored parameter row, not copied mixer state.
    pub(crate) fn prepare_sound_parameter_label(
        &mut self,
        name: &str,
        label: &str,
    ) -> Result<crate::sound_parameters::ParameterKey, Error> {
        let fail = |reason| self.inner.error(name, reason);
        if label.trim().is_empty() || label.contains('\0') {
            return Err(fail(Failure::SoundState(
                "empty or NUL-containing sound parameter label",
            )));
        }
        let bank = self
            .sound_parameters
            .as_ref()
            .ok_or_else(|| fail(Failure::SoundState("parameters not admitted")))?;
        if label.len() > bank.label_limit() {
            return Err(fail(Failure::Budget("sound parameter label")));
        }
        if let Some(index) = bank.find(label) {
            return Ok(index);
        }
        let Some((loader, budget)) = &mut self.sound_parameter_loader else {
            return Err(fail(Failure::MissingSoundLabel(label.into())));
        };
        let next = loader(label).map_err(|e| fail(Failure::SoundSource(e)))?;
        self.sound_parameters
            .as_mut()
            .ok_or_else(|| fail(Failure::SoundState("parameter banks required")))?
            .append_label(next, label, *budget)
            .map_err(|e| fail(Failure::SoundSource(e.to_string())))
    }
}

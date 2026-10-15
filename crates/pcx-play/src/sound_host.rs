use super::{Entry, Error, Failure, ObjectHost};
use crate::label_sounds::{BoundSound, SoundBank};
use pcx_script::{HandleId, Value};

#[derive(Debug)]
pub struct SoundState<'a> {
    pub identity: HandleId,
    pub resource: &'a BoundSound,
    /// Exact authored integers; no fade units, clamping or scheduling are inferred.
    pub fade_in_rate: i32,
    pub fade_out_rate: i32,
}
impl ObjectHost {
    pub fn with_label_sounds(mut self, bank: SoundBank) -> Result<Self, Error> {
        if self.sounds.is_some() {
            return Err(self
                .inner
                .error("CreateMIDISound", Failure::SoundState("already admitted")));
        }
        Self::contract(
            &self.inner,
            "CreateMIDISound",
            &["string", "integer", "integer"],
            "sound",
            false,
            Some("sound"),
        )?;
        self.sounds = Some(bank);
        Ok(self)
    }
    pub fn sound_bank(&self) -> Option<&SoundBank> {
        self.sounds.as_ref()
    }
    pub fn sound_state(&self, identity: HandleId) -> Result<SoundState<'_>, Error> {
        let fail = |reason| self.inner.error("CreateMIDISound", reason);
        let bank = self
            .sounds
            .as_ref()
            .ok_or_else(|| fail(Failure::SoundState("not admitted")))?;
        let Entry::Sound {
            resource,
            fade_in_rate,
            fade_out_rate,
        } = self.inner.entry(identity)?
        else {
            return Err(fail(Failure::WrongFamily));
        };
        Ok(SoundState {
            identity,
            resource: bank
                .sounds()
                .get(*resource)
                .ok_or_else(|| fail(Failure::Identity))?,
            fade_in_rate: *fade_in_rate,
            fade_out_rate: *fade_out_rate,
        })
    }
    pub(super) fn sound_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let bank = self.sounds.as_ref()?;
        if name != "CreateMIDISound" {
            return None;
        }
        let fail = |reason| self.inner.error(name, reason);
        let [
            Value::Str(label),
            Value::Int(fade_in_rate),
            Value::Int(fade_out_rate),
        ] = args
        else {
            return Some(Err(fail(Failure::SoundState(
                "expected label and two integer rates",
            ))));
        };
        if label.len() > bank.label_limit() {
            return Some(Err(fail(Failure::Budget("sound label"))));
        }
        let Some(resource) = bank.sounds().iter().position(|s| s.label() == label) else {
            return Some(Err(fail(Failure::MissingSoundLabel(label.clone()))));
        };
        Some(
            self.inner
                .allocate(
                    Entry::Sound {
                        resource,
                        fade_in_rate: *fade_in_rate,
                        fade_out_rate: *fade_out_rate,
                    },
                    name,
                )
                .map(Value::Handle),
        )
    }
}

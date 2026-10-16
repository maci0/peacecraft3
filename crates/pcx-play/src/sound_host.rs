use super::{Entry, Error, Failure, ObjectHost};
use crate::label_sounds::{BoundSound, PcmResource, SoundBank};
use pcx_script::{HandleId, Value};

#[derive(Debug)]
pub struct SoundState<'a> {
    pub identity: HandleId,
    /// Exact last SetSoundDuration argument; unset until explicitly assigned.
    pub duration: Option<i32>,
    pub resource: &'a BoundSound,
    /// Exact authored integers; no fade units, clamping or scheduling are inferred.
    pub fade_in_rate: i32,
    pub fade_out_rate: i32,
}
impl ObjectHost {
    /// Admits raw authored duration state, not a timer or playback-length override.
    pub fn with_sound_duration(mut self) -> Result<Self, Error> {
        let name = "SetSoundDuration";
        if self.sound_duration || (self.sounds.is_none() && self.file_sounds.is_none()) {
            return Err(self.inner.error(
                name,
                Failure::SoundState("sound resources required; duration must be admitted once"),
            ));
        }
        Self::contract(
            &self.inner,
            name,
            &["sound", "integer"],
            "nothing",
            false,
            Some("sound"),
        )?;
        self.sound_duration = true;
        Ok(self)
    }
    pub fn sound_duration(&self, id: HandleId) -> Result<Option<i32>, Error> {
        match self.inner.entry(id)? {
            Entry::Sound { duration, .. } | Entry::FileSound { duration, .. } => Ok(*duration),
            _ => Err(self.inner.error("SetSoundDuration", Failure::WrongFamily)),
        }
    }
    pub(super) fn sound_duration_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != "SetSoundDuration" || !self.sound_duration {
            return None;
        }
        Some((|| {
            let fail = |reason| self.inner.error(name, reason);
            let [Value::Handle(id), Value::Int(value)] = args else {
                return Err(fail(Failure::SoundState(
                    "expected sound identity and integer duration",
                )));
            };
            self.sound_admitted(*id).map_err(|e| fail(e.failure))?;
            let at = usize::try_from(id.token() - 1).map_err(|_| fail(Failure::Identity))?;
            let (Entry::Sound { duration, .. } | Entry::FileSound { duration, .. }) =
                &mut self.inner.arena[at]
            else {
                return Err(self.inner.error(name, Failure::WrongFamily));
            };
            *duration = Some(*value);
            Ok(Value::Null)
        })())
    }
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
    /// Queues sound starts for an explicit audio consumer. Admission does not
    /// install a playback device or infer channel, fade, looping or restart rules.
    pub fn with_sound_starts(mut self, max_pending: usize) -> Result<Self, Error> {
        let name = "StartSound";
        if (self.sounds.is_none() && self.file_sounds.is_none()) || self.sound_start_limit.is_some()
        {
            return Err(self.inner.error(
                name,
                Failure::SoundState("sound bank required; start queue must not be admitted twice"),
            ));
        }
        Self::contract(
            &self.inner,
            name,
            &["sound"],
            "nothing",
            false,
            Some("sound"),
        )?;
        std::alloc::Layout::array::<HandleId>(max_pending)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        self.sound_start_limit = Some(max_pending);
        Ok(self)
    }
    /// Every successful request remains in execution order, including repeats.
    pub fn pending_sound_starts(&self) -> &[HandleId] {
        &self.sound_starts
    }
    /// Transfers pending work to the consumer; sound identities/resources remain
    /// owned by this Host. The consumer owns retry/failure handling after transfer.
    pub fn take_sound_starts(&mut self) -> Vec<HandleId> {
        std::mem::take(&mut self.sound_starts)
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
            ..
        } = self.inner.entry(identity)?
        else {
            return Err(fail(Failure::WrongFamily));
        };
        Ok(SoundState {
            identity,
            duration: self.sound_duration(identity)?,
            resource: bank
                .sounds()
                .get(*resource)
                .ok_or_else(|| fail(Failure::Identity))?,
            fade_in_rate: *fade_in_rate,
            fade_out_rate: *fade_out_rate,
        })
    }
    pub(crate) fn sound_admitted(&self, identity: HandleId) -> Result<(), Error> {
        match self.inner.entry(identity)? {
            Entry::FileSound { mp3: true, .. } => self.file_mp3_state(identity).map(|_| ()),
            Entry::Sound { .. } | Entry::FileSound { mp3: false, .. } => {
                self.sound_resource(identity).map(|_| ())
            }
            _ => Err(self.inner.error("sound resource", Failure::WrongFamily)),
        }
    }
    /// Common checked PCM access for both label and direct-file sound objects.
    pub fn sound_resource(&self, identity: HandleId) -> Result<&PcmResource, Error> {
        match self.inner.entry(identity)? {
            Entry::Sound { .. } => Ok(self.sound_state(identity)?.resource.resource()),
            Entry::FileSound { .. } => Ok(self.file_sound_state(identity)?.resource),
            _ => Err(self.inner.error("sound resource", Failure::WrongFamily)),
        }
    }
    pub(super) fn sound_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name == "StartSound" {
            let limit = self.sound_start_limit?;
            return Some((|| {
                let [Value::Handle(id)] = args else {
                    return Err(self
                        .inner
                        .error(name, Failure::SoundState("expected sound identity")));
                };
                self.sound_admitted(*id)
                    .map_err(|e| self.inner.error(name, e.failure))?;
                if self.sound_starts.len() >= limit {
                    return Err(self
                        .inner
                        .error(name, Failure::Budget("pending sound starts")));
                }
                self.sound_starts
                    .try_reserve(1)
                    .map_err(|_| self.inner.error(name, Failure::Capacity))?;
                self.sound_starts.push(*id);
                Ok(Value::Null)
            })());
        }
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
                        properties: super::SoundProperties::default(),
                        parameters: None,
                        duration: None,
                        fade_in_rate: *fade_in_rate,
                        fade_out_rate: *fade_out_rate,
                    },
                    name,
                )
                .map(Value::Handle),
        )
    }
}

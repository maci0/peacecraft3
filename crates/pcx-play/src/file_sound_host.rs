use super::{Entry, Error, Failure, ObjectHost};
use crate::file_sounds::{Assets, Budget, Mp3Resource};
use crate::label_sounds::PcmResource;
use pcx_script::{HandleId, Value};
pub(super) struct State {
    assets: Assets,
    eax_bytes: usize,
    loader: Option<(FileSoundLoader, Budget)>,
}
type FileSoundLoader = Box<dyn FnMut(&str, Budget) -> Result<Assets, String> + Send + Sync>;
/// Exact creation inputs, without effective fade/channel/EAX or playback guesses.
#[derive(Debug)]
pub struct FileSoundState<'a, R = PcmResource> {
    pub identity: HandleId,
    /// Exact last SetSoundDuration argument; unset until explicitly assigned.
    pub duration: Option<i32>,
    pub resource: &'a R,
    pub looping: bool,
    pub is_3d: bool,
    pub stop_when_out_of_range: bool,
    pub fade_in_rate: i32,
    pub fade_out_rate: i32,
    pub eax_setting: &'a str,
}
impl ObjectHost {
    pub fn with_file_sounds(mut self, assets: Assets, max_eax_bytes: usize) -> Result<Self, Error> {
        let name = "CreateSound";
        if self.file_sounds.is_some() {
            return Err(self
                .inner
                .error(name, Failure::SoundState("file sounds already admitted")));
        }
        std::alloc::Layout::array::<u8>(max_eax_bytes)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        Self::contract(
            &self.inner,
            name,
            &[
                "string", "boolean", "boolean", "boolean", "integer", "integer", "string",
            ],
            "sound",
            false,
            Some("sound"),
        )?;
        if self
            .inner
            .program
            .natives()
            .get("CreateSoundFilenameWithLabel")
            .is_some()
        {
            Self::contract(
                &self.inner,
                "CreateSoundFilenameWithLabel",
                &[
                    "string", "boolean", "boolean", "boolean", "integer", "integer", "string",
                ],
                "sound",
                false,
                Some("sound"),
            )?;
            let natives = self.inner.program.natives();
            let same_source = natives.get(name).map(|n| &n.source)
                == natives
                    .get("CreateSoundFilenameWithLabel")
                    .map(|n| &n.source);
            if !same_source {
                return Err(self
                    .inner
                    .error("CreateSoundFilenameWithLabel", Failure::Source));
            }
        }
        self.file_sounds = Some(State {
            assets,
            eax_bytes: max_eax_bytes,
            loader: None,
        });
        Ok(self)
    }
    /// Load missing executed filenames; existing resources and identities stay in place.
    /// The loader receives the bank's remaining input budget. It must return exactly
    /// the requested binding. Decoder work and source-read attempts also need bounds.
    pub fn admit_file_sound_loader(
        &mut self,
        budget: Budget,
        loader: impl FnMut(&str, Budget) -> Result<Assets, String> + Send + Sync + 'static,
    ) -> Result<(), Error> {
        let fail = |reason| self.inner.error("CreateSound", reason);
        let state = self
            .file_sounds
            .as_mut()
            .ok_or_else(|| fail(Failure::SoundState("file sounds not admitted")))?;
        if state.loader.is_some() {
            return Err(fail(Failure::SoundState(
                "file sound loader already admitted",
            )));
        }
        state
            .assets
            .admit_budget(budget)
            .map_err(|error| fail(Failure::SoundSource(error.to_string())))?;
        state.loader = Some((Box::new(loader), budget));
        Ok(())
    }
    pub fn file_sound_assets(&self) -> Option<&Assets> {
        Some(&self.file_sounds.as_ref()?.assets)
    }
    pub fn file_sound_state(&self, identity: HandleId) -> Result<FileSoundState<'_>, Error> {
        let fail = |reason| self.inner.error("CreateSound", reason);
        let state = self
            .file_sounds
            .as_ref()
            .ok_or_else(|| fail(Failure::SoundState("file sounds not admitted")))?;
        let Entry::FileSound {
            resource,
            mp3,
            looping,
            is_3d,
            stop_when_out_of_range,
            fade_in_rate,
            fade_out_rate,
            eax_setting,
            ..
        } = self.inner.entry(identity)?
        else {
            return Err(fail(Failure::WrongFamily));
        };
        if *mp3 {
            return Err(fail(Failure::SoundState("integer PCM required")));
        }
        Ok(FileSoundState {
            identity,
            duration: self.sound_duration(identity)?,
            resource: state
                .assets
                .resources()
                .get(*resource)
                .ok_or_else(|| fail(Failure::Identity))?,
            looping: *looping,
            is_3d: *is_3d,
            stop_when_out_of_range: *stop_when_out_of_range,
            fade_in_rate: *fade_in_rate,
            fade_out_rate: *fade_out_rate,
            eax_setting,
        })
    }
    pub fn file_mp3_state(
        &self,
        identity: HandleId,
    ) -> Result<FileSoundState<'_, Mp3Resource>, Error> {
        let fail = |reason| self.inner.error("CreateSound", reason);
        let duration = self.sound_duration(identity)?;
        let state = self
            .file_sounds
            .as_ref()
            .ok_or_else(|| fail(Failure::SoundState("file sounds not admitted")))?;
        let Entry::FileSound {
            resource,
            mp3,
            looping,
            is_3d,
            stop_when_out_of_range,
            fade_in_rate,
            fade_out_rate,
            eax_setting,
            ..
        } = self.inner.entry(identity)?
        else {
            return Err(fail(Failure::WrongFamily));
        };
        if !*mp3 {
            return Err(fail(Failure::SoundState("MPEG audio required")));
        }
        let mp3 = state
            .assets
            .mp3_resources()
            .get(*resource)
            .ok_or_else(|| fail(Failure::Identity))?;
        Ok(FileSoundState {
            identity,
            duration,
            resource: mp3,
            looping: *looping,
            is_3d: *is_3d,
            stop_when_out_of_range: *stop_when_out_of_range,
            fade_in_rate: *fade_in_rate,
            fade_out_rate: *fade_out_rate,
            eax_setting,
        })
    }
    pub(super) fn file_sound_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if self.file_sounds.is_none() {
            return None;
        }
        let labeled = name == "CreateSoundFilenameWithLabel";
        if name != "CreateSound" && !labeled {
            return None;
        }
        if labeled && self.inner.program.natives().get(name).is_none() {
            return None;
        }
        let fail = |reason| self.inner.error(name, reason);
        let [
            Value::Str(member),
            Value::Bool(looping),
            Value::Bool(is_3d),
            Value::Bool(stop_when_out_of_range),
            Value::Int(fade_in_rate),
            Value::Int(fade_out_rate),
            Value::Str(tail),
        ] = args
        else {
            return Some(Err(fail(Failure::SoundState(if labeled {
                "expected filename, three booleans, two integer rates and sound label"
            } else {
                "expected filename, three booleans, two integer rates and EAX text"
            }))));
        };
        let (member, tail) = (member.clone(), tail.clone());
        let (looping, is_3d, stop_when_out_of_range, fade_in_rate, fade_out_rate) = (
            *looping,
            *is_3d,
            *stop_when_out_of_range,
            *fade_in_rate,
            *fade_out_rate,
        );
        let state = self.file_sounds.as_ref().expect("admitted");
        let eax = if labeled { "" } else { tail.as_str() };
        if member.len() > state.assets.path_limit() || eax.len() > state.eax_bytes {
            return Some(Err(fail(Failure::Budget("file sound text"))));
        }
        if member.contains('\0') || eax.contains('\0') {
            return Some(Err(fail(Failure::SoundState(
                "embedded NUL in filename or EAX text is unsupported",
            ))));
        }
        // The label selects an admitted parameter row. It is not EAX text.
        let parameters = if labeled {
            match self.prepare_sound_parameter_label(name, &tail) {
                Ok(index) => Some(index),
                Err(error) => return Some(Err(error)),
            }
        } else {
            None
        };
        let state = self.file_sounds.as_mut().expect("admitted");
        Some((|| {
            let fail = |reason| self.inner.error(name, reason);
            let (resource, mp3) = match state.assets.find(&member) {
                Some(binding) => binding,
                None => {
                    let Some((loader, budget)) = &mut state.loader else {
                        return Err(fail(Failure::MissingSoundFile(member.clone())));
                    };
                    let remaining = state
                        .assets
                        .remaining_budget(*budget)
                        .map_err(|error| fail(Failure::SoundSource(error.to_string())))?;
                    if remaining.resources == 0
                        || remaining.total_bytes == 0
                        || remaining.metadata_bytes == 0
                    {
                        return Err(fail(Failure::Budget("file sound bank")));
                    }
                    let next = loader(&member, remaining)
                        .map_err(|error| fail(Failure::SoundSource(error)))?;
                    if next.resources().len() + next.mp3_resources().len() != 1
                        || next.find(&member).is_none()
                    {
                        return Err(fail(Failure::SoundState(
                            "loader must return exactly the requested file",
                        )));
                    }
                    state
                        .assets
                        .append(next, *budget)
                        .map_err(|error| fail(Failure::SoundSource(error.to_string())))?;
                    state
                        .assets
                        .find(&member)
                        .ok_or_else(|| fail(Failure::MissingSoundFile(member.clone())))?
                }
            };
            let mut eax_setting = String::new();
            eax_setting
                .try_reserve_exact(eax.len())
                .map_err(|_| fail(Failure::Capacity))?;
            eax_setting.push_str(eax);
            self.inner
                .allocate(
                    Entry::FileSound {
                        resource,
                        properties: super::SoundProperties::default(),
                        mp3,
                        parameters,
                        duration: None,
                        looping,
                        is_3d,
                        stop_when_out_of_range,
                        fade_in_rate,
                        fade_out_rate,
                        eax_setting,
                    },
                    name,
                )
                .map(Value::Handle)
        })())
    }
}

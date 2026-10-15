use super::{Entry, Error, Failure, ObjectHost};
use crate::file_sounds::Assets;
use crate::label_sounds::PcmResource;
use pcx_script::{HandleId, Value};
pub(super) struct State {
    assets: Assets,
    eax_bytes: usize,
}
/// Exact creation inputs, without effective fade/channel/EAX or playback guesses.
#[derive(Debug)]
pub struct FileSoundState<'a> {
    pub identity: HandleId,
    pub resource: &'a PcmResource,
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
        self.file_sounds = Some(State {
            assets,
            eax_bytes: max_eax_bytes,
        });
        Ok(self)
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
        Ok(FileSoundState {
            identity,
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
    pub(super) fn file_sound_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let state = self.file_sounds.as_ref()?;
        if name != "CreateSound" {
            return None;
        }
        Some((|| {
            let fail = |reason| self.inner.error(name, reason);
            let [
                Value::Str(member),
                Value::Bool(looping),
                Value::Bool(is_3d),
                Value::Bool(stop_when_out_of_range),
                Value::Int(fade_in_rate),
                Value::Int(fade_out_rate),
                Value::Str(eax),
            ] = args
            else {
                return Err(fail(Failure::SoundState(
                    "expected filename, three booleans, two integer rates and EAX text",
                )));
            };
            if member.len() > state.assets.path_limit() || eax.len() > state.eax_bytes {
                return Err(fail(Failure::Budget("file sound text")));
            }
            if eax.contains('\0') {
                return Err(fail(Failure::SoundState(
                    "embedded NUL in EAX text is unsupported",
                )));
            }
            let resource = state
                .assets
                .resources()
                .iter()
                .position(|r| r.member() == member)
                .ok_or_else(|| fail(Failure::MissingSoundFile(member.clone())))?;
            let mut eax_setting = String::new();
            eax_setting
                .try_reserve_exact(eax.len())
                .map_err(|_| fail(Failure::Capacity))?;
            eax_setting.push_str(eax);
            self.inner
                .allocate(
                    Entry::FileSound {
                        resource,
                        parameters: None,
                        looping: *looping,
                        is_3d: *is_3d,
                        stop_when_out_of_range: *stop_when_out_of_range,
                        fade_in_rate: *fade_in_rate,
                        fade_out_rate: *fade_out_rate,
                        eax_setting,
                    },
                    name,
                )
                .map(Value::Handle)
        })())
    }
}

//! Ordered presentation intents; the application owns the audio epoch and wall clock.
use super::{ObjectHost, Error, Failure, controller::Operation, rules::Rules};
use pcx_script::Value;
use std::sync::Arc;

// Recreation memory bound, independent of an authored playlist or callback limit.
const MAX_COMMANDS: usize = 65_536;

impl ObjectHost {
    pub fn with_regular_music_rules(mut self, rules: Result<Rules, String>) -> Result<Self, Error> {
        let fail = |reason| self.inner.error("PlayMusic", Failure::MusicState(reason));
        let state = self.play_music.as_mut().ok_or_else(|| fail("play music is not admitted"))?;
        if state.rules.is_some() || !state.commands.is_empty() {
            return Err(fail("regular music rules already configured"));
        }
        state.rules = Some(rules.map(Arc::new));
        Ok(self)
    }

    pub fn regular_music_commands(&self) -> Option<&[Operation]> {
        self.regular_music_ready().then(|| self.play_music.as_ref().unwrap().commands.as_slice())
    }
    pub fn regular_music_rules(&self) -> Option<Result<&Arc<Rules>, &str>> {
        self.play_music.as_ref()?.rules.as_ref().map(|r| r.as_ref().map_err(String::as_str))
    }
    pub(super) fn regular_music_ready(&self) -> bool {
        self.play_music.as_ref().is_some_and(|s| matches!((&s.rules, &s.volume_rules), (Some(Ok(_)), Some(Ok(_)))))
    }
    pub(super) fn append_music(&mut self, name: &str, operation: Operation) -> Result<(), Error> {
        if !self.regular_music_ready() { return Ok(()); }
        let fail = |reason| self.inner.error(name, reason);
        let state = self.play_music.as_mut().unwrap();
        if state.commands.len() >= MAX_COMMANDS {
            return Err(fail(Failure::Budget("ordered music commands")));
        }
        state.commands.try_reserve(1).map_err(|_| fail(Failure::Capacity))?;
        state.commands.push(operation);
        Ok(())
    }
    pub(super) fn regular_music_native(&mut self, name: &str, args: &[Value]) -> Option<Result<Value, Error>> {
        let state = self.play_music.as_ref()?;
        if state.rules.is_none() || (name == "SetMusicVolume" && !self.regular_music_ready()) || !matches!(name, "PlayMusic" | "PlayMusicEx" | "StopMusic" | "ResumeMusic" | "SetMusicPlayPosition" | "SetMusicVolume") { return None; }
        Some((|| {
            let fail = |why| self.inner.error(name, why);
            let rules = state.rules.as_ref().unwrap().as_ref().map_err(|why| fail(Failure::SoundSource(why.clone())))?;
            let backend = state.volume_rules.as_ref().ok_or_else(|| fail(Failure::MusicState("regular music backend rules are missing")))?;
            let backend = backend.as_ref().map_err(|why| fail(Failure::SoundSource(why.clone())))?;
            if backend.image_sha256 != rules.image_sha256 {
                return Err(fail(Failure::MusicState("music policies use different client images")));
            }
            let position = |value: i32| u32::try_from(value).map_err(|_| fail(Failure::MusicState("negative regular music offset or fade is unsupported")));
            let music_name = |text: &pcx_script::ScriptString| super::checked_name(text, state.max_name_bytes.min(rules.name_bytes.saturating_sub(1)), &fail);
            let op = match (name, args) {
                ("PlayMusic", [Value::Str(text)]) => Operation::Play { name: music_name(text)?, position_ms: 0, fade_ms: 0 },
                ("PlayMusicEx", [Value::Str(text), Value::Int(from), Value::Int(fade)]) => Operation::Play { name: music_name(text)?, position_ms: position(*from)?, fade_ms: position(*fade)? },
                ("StopMusic", [Value::Bool(fade)]) => Operation::Stop { fade: *fade },
                ("ResumeMusic", []) => Operation::Resume,
                ("SetMusicPlayPosition", [Value::Int(at)]) => Operation::Seek(position(*at)?),
                ("SetMusicVolume", [Value::Int(level)]) => Operation::Volume(backend.level(*level).map_err(|why| fail(Failure::SoundSource(why)))?),
                _ => return Err(fail(Failure::MusicState("expected the declared music arguments"))),
            };
            let volume = if let Operation::Volume(level) = &op {
                let old = state.volume_request.as_ref().ok_or_else(|| fail(Failure::MusicState("regular volume state is absent")))?;
                Some(super::RegularMusicVolumeRequest { rules: Arc::clone(&old.rules), level: *level, revision: old.revision.checked_add(1).ok_or_else(|| fail(Failure::Budget("regular music volume revision")))? })
            } else { None };
            self.append_music(name, op)?;
            if let Some(volume) = volume { let state = self.play_music.as_mut().unwrap(); state.volume = Some(volume.level as i32); state.volume_request = Some(volume); }
            Ok(Value::Null)
        })())
    }
}

//! Admitted `common.j` music-playback declarations and thematic dispatch.
//!
//! The eight natives keep their shipped declarations verbatim (all
//! non-constant; the `frommsecs` parameter spelling is the shipped typo and is
//! not corrected here). Source-qualified thematic calls update the shared
//! map-music request. Regular volume additionally requires the selected client and
//! backend policy. Other calls retain their arguments and refuse until consumed.
use super::{Error, Failure, ObjectHost};
use pcx_script::Value;
use std::sync::Arc;

#[path = "regular_music_volume_rules.rs"]
pub(crate) mod volume_rules;

#[derive(Clone, Debug)]
pub struct RegularMusicVolumeRequest {
    pub rules: Arc<volume_rules::Rules>,
    pub level: u32,
    pub revision: u64,
}

/// Exact executed `PlayMusic`/`PlayMusicEx` arguments, not a playlist or voice.
/// `None` comes from `PlayMusic`, which declares no timing arguments; `Some`
/// comes from `PlayMusicEx`. No zero default is invented for the missing case.
#[derive(Debug, PartialEq, Eq)]
pub struct PlayMusicRequest {
    pub name: String,
    pub from_msecs: Option<i32>,
    pub fadein_msecs: Option<i32>,
}

/// Exact executed `PlayThematicMusicEx` arguments, not a resolved track.
#[derive(Debug, PartialEq, Eq)]
pub struct ThematicMusicExRequest {
    pub path: String,
    pub from_msecs: i32,
}

pub(super) struct State {
    max_name_bytes: usize,
    request: Option<PlayMusicRequest>,
    stop_fade_out: Option<bool>,
    resumes: u64,
    volume: Option<i32>,
    volume_rules: Option<Result<Arc<volume_rules::Rules>, String>>,
    volume_request: Option<RegularMusicVolumeRequest>,
    play_position: Option<i32>,
    thematic_ex: Option<ThematicMusicExRequest>,
    thematic_play_position: Option<i32>,
}

fn checked_name(
    text: &str,
    max_name_bytes: usize,
    fail: &dyn Fn(Failure) -> Error,
) -> Result<String, Error> {
    if text.len() > max_name_bytes {
        return Err(fail(Failure::Budget("play music name bytes")));
    }
    if text.contains('\0') {
        return Err(fail(Failure::MusicState("embedded NUL is unsupported")));
    }
    let mut name = String::new();
    name.try_reserve_exact(text.len())
        .map_err(|_| fail(Failure::Capacity))?;
    name.push_str(text);
    Ok(name)
}

fn is_play_music_native(name: &str) -> bool {
    matches!(
        name,
        "PlayMusic"
            | "PlayMusicEx"
            | "StopMusic"
            | "ResumeMusic"
            | "PlayThematicMusicEx"
            | "SetMusicVolume"
            | "SetMusicPlayPosition"
            | "SetThematicMusicPlayPosition"
    )
}

impl ObjectHost {
    /// Admits declarations and request retention. Thematic playback additionally
    /// requires map-music admission and selected-client thematic rules.
    pub fn with_play_music(mut self, max_name_bytes: usize) -> Result<Self, Error> {
        let name = "PlayMusic";
        if self.play_music.is_some() {
            return Err(self
                .inner
                .error(name, Failure::MusicState("already admitted")));
        }
        std::alloc::Layout::array::<u8>(max_name_bytes)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        // The eight natives form one playback surface: when the prelude
        // declares any of them, all eight keep the full contract, exactly as
        // the reviewed common.j declares them. Synthetic or custom preludes
        // without any music surface skip the block instead of failing, as
        // with the sibling optional natives; undeclared names never reach
        // dispatch because script resolution rejects unknown names first.
        let any = [
            "PlayMusic",
            "PlayMusicEx",
            "StopMusic",
            "ResumeMusic",
            "PlayThematicMusicEx",
            "SetMusicVolume",
            "SetMusicPlayPosition",
            "SetThematicMusicPlayPosition",
        ]
        .iter()
        .any(|native| self.inner.program.natives().get(native).is_some());
        if any {
            for (native, args) in [
                ("PlayMusic", &["string"][..]),
                ("PlayMusicEx", &["string", "integer", "integer"][..]),
                ("StopMusic", &["boolean"][..]),
                ("ResumeMusic", &[][..]),
                ("PlayThematicMusicEx", &["string", "integer"][..]),
                ("SetMusicVolume", &["integer"][..]),
                ("SetMusicPlayPosition", &["integer"][..]),
                ("SetThematicMusicPlayPosition", &["integer"][..]),
            ] {
                Self::contract(&self.inner, native, args, "nothing", false, None)?;
            }
            let source = self
                .inner
                .program
                .natives()
                .get(name)
                .expect("checked contract")
                .source
                .clone();
            if self
                .inner
                .program
                .natives()
                .get("CreateTimer")
                .is_none_or(|timer| timer.source != source)
            {
                return Err(self.inner.error(name, Failure::Source));
            }
            for native in [
                "PlayMusicEx",
                "StopMusic",
                "ResumeMusic",
                "PlayThematicMusicEx",
                "SetMusicVolume",
                "SetMusicPlayPosition",
                "SetThematicMusicPlayPosition",
            ] {
                if self
                    .inner
                    .program
                    .natives()
                    .get(native)
                    .expect("checked contract")
                    .source
                    != source
                {
                    return Err(self.inner.error(native, Failure::Source));
                }
            }
        }
        self.play_music = Some(State {
            max_name_bytes,
            request: None,
            stop_fade_out: None,
            resumes: 0,
            volume: None,
            volume_rules: None,
            volume_request: None,
            play_position: None,
            thematic_ex: None,
            thematic_play_position: None,
        });
        Ok(self)
    }
    /// Missing backend policy is retained until the volume native is called.
    pub fn with_regular_music_volume_rules(
        mut self,
        rules: Result<volume_rules::Rules, String>,
    ) -> Result<Self, Error> {
        let fail = |reason| {
            self.inner
                .error("SetMusicVolume", Failure::MusicState(reason))
        };
        let state = self
            .play_music
            .as_mut()
            .ok_or_else(|| fail("play music is not admitted"))?;
        if state.volume_rules.is_some() {
            return Err(fail("regular music volume already configured"));
        }
        let rules = rules.map(Arc::new);
        if let Ok(rules) = &rules {
            state.volume_request = Some(RegularMusicVolumeRequest {
                level: rules.initial_level(),
                rules: Arc::clone(rules),
                revision: 0,
            });
        }
        state.volume_rules = Some(rules);
        Ok(self)
    }

    pub fn regular_music_volume_request(&self) -> Option<&RegularMusicVolumeRequest> {
        self.play_music.as_ref()?.volume_request.as_ref()
    }

    /// Last executed `PlayMusic`/`PlayMusicEx` request, if any.
    pub fn play_music_request(&self) -> Option<&PlayMusicRequest> {
        self.play_music.as_ref()?.request.as_ref()
    }
    /// Last executed `StopMusic` fade-out flag, if any.
    pub fn stop_music_fade_out(&self) -> Option<bool> {
        self.play_music.as_ref()?.stop_fade_out
    }
    /// Executed `ResumeMusic` count since admission, if admitted.
    pub fn resume_music_requests(&self) -> Option<u64> {
        self.play_music.as_ref().map(|state| state.resumes)
    }
    /// Last executed `SetMusicVolume` argument, if any.
    pub fn music_volume_request(&self) -> Option<i32> {
        self.play_music.as_ref()?.volume
    }
    /// Last executed `SetMusicPlayPosition` argument, if any.
    pub fn music_play_position_request(&self) -> Option<i32> {
        self.play_music.as_ref()?.play_position
    }
    /// Last unqualified `PlayThematicMusicEx` request, retained before refusal.
    /// Source-qualified calls use `thematic_music_request()` instead.
    pub fn thematic_music_ex_request(&self) -> Option<&ThematicMusicExRequest> {
        self.play_music.as_ref()?.thematic_ex.as_ref()
    }
    /// Last unqualified thematic seek, retained before refusal.
    pub fn thematic_play_position_request(&self) -> Option<i32> {
        self.play_music.as_ref()?.thematic_play_position
    }
    pub(super) fn play_music_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.play_music.as_ref()?;
        match (name, args) {
            ("PlayThematicMusicEx", [Value::Str(path), Value::Int(from)])
                if self
                    .map_music
                    .as_ref()
                    .is_some_and(|state| state.has_thematic_rules()) =>
            {
                return Some(self.start_thematic(path, Some(*from)).map(|()| Value::Null));
            }
            ("SetThematicMusicPlayPosition", [Value::Int(position)])
                if self
                    .map_music
                    .as_ref()
                    .is_some_and(|state| state.has_thematic_rules()) =>
            {
                return Some(self.seek_thematic(*position).map(|()| Value::Null));
            }
            _ => {}
        }
        let state = self.play_music.as_mut().expect("checked music admission");
        if !is_play_music_native(name) {
            return None;
        }
        Some((|| {
            let fail = |reason| self.inner.error(name, reason);
            match (name, args) {
                ("PlayMusic", [Value::Str(text)]) => {
                    state.request = Some(PlayMusicRequest {
                        name: checked_name(text, state.max_name_bytes, &fail)?,
                        from_msecs: None,
                        fadein_msecs: None,
                    });
                }
                ("PlayMusicEx", [Value::Str(text), Value::Int(from), Value::Int(fade)]) => {
                    state.request = Some(PlayMusicRequest {
                        name: checked_name(text, state.max_name_bytes, &fail)?,
                        from_msecs: Some(*from),
                        fadein_msecs: Some(*fade),
                    });
                }
                ("StopMusic", [Value::Bool(fade_out)]) => {
                    state.stop_fade_out = Some(*fade_out);
                }
                ("ResumeMusic", []) => {
                    state.resumes = state.resumes.saturating_add(1);
                }
                ("PlayThematicMusicEx", [Value::Str(path), Value::Int(from)]) => {
                    state.thematic_ex = Some(ThematicMusicExRequest {
                        path: checked_name(path, state.max_name_bytes, &fail)?,
                        from_msecs: *from,
                    });
                }
                ("SetMusicVolume", [Value::Int(volume)]) => {
                    if let Some(rules) = state
                        .volume_rules
                        .as_ref()
                        .filter(|_| self.inner.program.natives().get(name).is_some())
                    {
                        let rules = rules
                            .as_ref()
                            .map_err(|error| fail(Failure::SoundSource(error.clone())))?;
                        let level = rules
                            .level(*volume)
                            .map_err(|error| fail(Failure::SoundSource(error)))?;
                        let revision = state
                            .volume_request
                            .as_ref()
                            .ok_or_else(|| {
                                fail(Failure::MusicState("regular volume state is absent"))
                            })?
                            .revision
                            .checked_add(1)
                            .ok_or_else(|| {
                                fail(Failure::Budget("regular music volume revision"))
                            })?;
                        state.volume_request = Some(RegularMusicVolumeRequest {
                            rules: Arc::clone(rules),
                            level,
                            revision,
                        });
                        state.volume = Some(*volume);
                        return Ok(Value::Null);
                    }
                    state.volume = Some(*volume);
                }
                ("SetMusicPlayPosition", [Value::Int(position)]) => {
                    state.play_position = Some(*position);
                }
                ("SetThematicMusicPlayPosition", [Value::Int(position)]) => {
                    state.thematic_play_position = Some(*position);
                }
                _ => {
                    return Err(fail(Failure::MusicState(
                        "expected the declared music arguments",
                    )));
                }
            }
            // Follow-up: audible playback (and the volume/position/fade
            // semantics it needs) is unqualified, so a consumer must keep
            // refusing loudly instead of guessing gains, offsets or fade
            // curves. See `docs/re-notes/campaign-audio.md` ("Other music
            // natives" rows and "Other music natives, music fades" status).
            Err(fail(Failure::MusicState(
                "audible music playback is unqualified; see docs/re-notes/campaign-audio.md",
            )))
        })())
    }
}

#[cfg(test)]
#[path = "play_music_host_tests.rs"]
mod tests;

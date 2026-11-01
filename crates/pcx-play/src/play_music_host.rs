//! Admitted `common.j` music-playback natives with retained requests only.
//!
//! The eight natives keep their shipped declarations verbatim (all
//! non-constant; the `frommsecs` parameter spelling is the shipped typo and is
//! not corrected here). Audible playback — volume scaling, seek positions and
//! fade timing — is unqualified, so dispatch retains the exact executed
//! arguments, surfaces them through the getters below, and refuses loudly
//! instead of guessing gains, offsets or curves.
use super::{Error, Failure, ObjectHost};
use pcx_script::Value;

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
    /// Admits request retention only. Every native keeps its shipped
    /// declaration (see the task's `common.j` extract); audible behavior stays
    /// an explicit dispatch refusal until the semantics are qualified.
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
            play_position: None,
            thematic_ex: None,
            thematic_play_position: None,
        });
        Ok(self)
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
    /// Last executed `PlayThematicMusicEx` request, if any.
    pub fn thematic_music_ex_request(&self) -> Option<&ThematicMusicExRequest> {
        self.play_music.as_ref()?.thematic_ex.as_ref()
    }
    /// Last executed `SetThematicMusicPlayPosition` argument, if any.
    pub fn thematic_play_position_request(&self) -> Option<i32> {
        self.play_music.as_ref()?.thematic_play_position
    }
    pub(super) fn play_music_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let state = self.play_music.as_mut()?;
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

//! Executed transmission scenes (`SetCinematicScene`/`EndCinematicScene`) for
//! the transmission panel: portrait rawcode, player color, speaker, text and
//! durations exactly as executed, with the game time the scene began.
use super::{Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct CinematicScene {
    pub portrait: i32,
    pub color: HandleId,
    pub speaker: String,
    pub text: String,
    pub scene_duration: f32,
    pub voiceover_duration: f32,
    pub started: f64,
}

impl CinematicScene {
    /// Rendered playback end: the longer of the authored scene and voiceover
    /// durations after the executed start. Non-finite or negative durations
    /// refuse; the renderer must not invent a fallback pose.
    pub fn ends_at(&self) -> Result<f64, String> {
        for (name, value) in [
            ("scene_duration", self.scene_duration),
            ("voiceover_duration", self.voiceover_duration),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!("cinematic {name} duration is not admitted"));
            }
        }
        Ok(self.started + f64::from(self.scene_duration.max(self.voiceover_duration)))
    }

    /// Whether `now` is still inside the authored playback window.
    pub fn active_at(&self, now: f64) -> Result<bool, String> {
        Ok(now < self.ends_at()?)
    }

    /// Whether the authored playback window has elapsed at `now`.
    pub fn completed_at(&self, now: f64) -> Result<bool, String> {
        self.active_at(now).map(|active| !active)
    }

    /// Bounded playback progress in `[0, 1]` for the renderer. A zero-length
    /// scene reports completion once started; time before start clamps to zero.
    pub fn progress_at(&self, now: f64) -> Result<f32, String> {
        let end = self.ends_at()?;
        let length = end - self.started;
        if length <= 0.0 {
            return Ok(if now < self.started { 0.0 } else { 1.0 });
        }
        Ok(((now - self.started) / length).clamp(0.0, 1.0) as f32)
    }
}

const SET: &str = "SetCinematicScene";
const END: &str = "EndCinematicScene";

impl ObjectHost {
    pub fn with_cinematic_scenes(mut self) -> Result<Self, Error> {
        if let Some(native) = self.inner.program.natives().get(SET) {
            let source = native.source.clone();
            Self::contract_in(
                &self.inner,
                SET,
                &["integer", "playercolor", "string", "string", "real", "real"],
                "nothing",
                false,
                None,
                false,
            )?;
            if let Some(end) = self.inner.program.natives().get(END) {
                if end.source != source {
                    return Err(self.inner.error(END, Failure::Source));
                }
                Self::contract_in(&self.inner, END, &[], "nothing", false, None, false)?;
            }
        }
        self.cinematic_scene = Some(None);
        Ok(self)
    }

    pub fn cinematic_scene(&self) -> Option<&CinematicScene> {
        self.cinematic_scene.as_ref()?.as_ref()
    }

    /// Rendered playback gate for the transmission panel: the stored scene
    /// plus its authored progress at the current game clock. Absent scene or
    /// clock stays `None`; malformed durations refuse at the call site.
    pub fn cinematic_playback(&self) -> Option<Result<(CinematicScene, f32), String>> {
        let scene = self.cinematic_scene()?.clone();
        let now = self.timer_game_time().unwrap_or(0.0);
        Some(scene.progress_at(now).map(|progress| (scene, progress)))
    }

    pub(super) fn cinematic_scene_native(
        &mut self,
        name: &str,
        args: &[Value],
        strings: Option<&pcx_script::text::NativeStringLookup<'_>>,
    ) -> Option<Result<Value, Error>> {
        self.cinematic_scene.as_ref()?;
        if !matches!(name, SET | END) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        let scene = match (name, args) {
            (END, []) => None,
            (
                SET,
                [
                    Value::Int(portrait),
                    Value::Handle(color),
                    Value::Str(speaker),
                    Value::Str(text),
                    scene,
                    voiceover,
                ],
            ) => {
                let (Some(scene_duration), Some(voiceover_duration)) =
                    (scene.as_real(), voiceover.as_real())
                else {
                    return Some(Err(self.inner.error(name, Failure::Signature)));
                };
                let speaker = match pcx_script::text::native_map_string(speaker, strings) {
                    Ok(text) => text.into_owned(),
                    Err(why) => {
                        return Some(Err(self.inner.error(name, Failure::ScriptString(why))));
                    }
                };
                let text = match pcx_script::text::native_map_string(text, strings) {
                    Ok(text) => text.into_owned(),
                    Err(why) => {
                        return Some(Err(self.inner.error(name, Failure::ScriptString(why))));
                    }
                };
                Some(CinematicScene {
                    portrait: *portrait,
                    color: *color,
                    speaker,
                    text,
                    scene_duration,
                    voiceover_duration,
                    started: self.timer_game_time().unwrap_or(0.0),
                })
            }
            _ => return Some(Err(self.inner.error(name, Failure::Signature))),
        };
        self.cinematic_scene = Some(scene);
        Some(Ok(Value::Null))
    }
}

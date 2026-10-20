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

    pub(super) fn cinematic_scene_native(
        &mut self,
        name: &str,
        args: &[Value],
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
                Some(CinematicScene {
                    portrait: *portrait,
                    color: *color,
                    speaker: speaker.clone(),
                    text: text.clone(),
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

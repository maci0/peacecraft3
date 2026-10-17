use super::{Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackedSoundAction {
    Register,
    Unregister,
}

/// A requested ambient registration, not proof that an audio device applied it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StackedSoundRequest {
    pub action: StackedSoundAction,
    pub sound: HandleId,
    pub by_position: bool,
    pub rect_width: f32,
    pub rect_height: f32,
    /// Last explicitly assigned position at the instant of this request.
    pub position: Option<[f32; 3]>,
}

impl ObjectHost {
    pub fn with_stacked_sounds(mut self, max_pending: usize) -> Result<Self, Error> {
        if self.stacked_sound_limit.is_some() || !self.has_sound_resources() {
            return Err(self.inner.error(
                "RegisterStackedSound",
                Failure::SoundState(
                    "sound resources required; stacked queue must be admitted once",
                ),
            ));
        }
        for name in ["RegisterStackedSound", "UnregisterStackedSound"] {
            Self::contract(
                &self.inner,
                name,
                &["sound", "boolean", "real", "real"],
                "nothing",
                false,
                Some("sound"),
            )?;
        }
        std::alloc::Layout::array::<StackedSoundRequest>(max_pending)
            .map_err(|_| self.inner.error("RegisterStackedSound", Failure::Capacity))?;
        self.stacked_sound_limit = Some(max_pending);
        Ok(self)
    }

    /// Includes duplicates and unregister requests, in actual execution order.
    pub fn pending_stacked_sound_requests(&self) -> &[StackedSoundRequest] {
        &self.stacked_sound_requests
    }

    // shortcut: starts use a separate queue; unify ordering when attaching an audio consumer.
    pub fn take_stacked_sound_requests(&mut self) -> Vec<StackedSoundRequest> {
        std::mem::take(&mut self.stacked_sound_requests)
    }

    pub(super) fn stacked_sound_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let limit = self.stacked_sound_limit?;
        let action = match name {
            "RegisterStackedSound" => StackedSoundAction::Register,
            "UnregisterStackedSound" => StackedSoundAction::Unregister,
            _ => return None,
        };
        Some((|| {
            let fail = |reason| self.inner.error(name, reason);
            let [Value::Handle(id), Value::Bool(by_position), width, height] = args else {
                return Err(fail(Failure::SoundState(
                    "expected sound, boolean and two real dimensions",
                )));
            };
            self.sound_admitted(*id).map_err(|e| fail(e.failure))?;
            let dimensions =
                [width, height].map(|v| v.as_real().filter(|v| v.is_finite() && *v >= 0.0));
            let [Some(rect_width), Some(rect_height)] = dimensions else {
                return Err(fail(Failure::SoundState(
                    "finite nonnegative rectangle dimensions required",
                )));
            };
            let position = self.sound_properties(*id)?.position;
            if *by_position && position.is_none() {
                return Err(fail(Failure::SoundState(
                    "position-based registration requires an explicit position",
                )));
            }
            if self.stacked_sound_requests.len() >= limit {
                return Err(fail(Failure::Budget("pending stacked sound requests")));
            }
            self.stacked_sound_requests
                .try_reserve(1)
                .map_err(|_| fail(Failure::Capacity))?;
            self.stacked_sound_requests.push(StackedSoundRequest {
                action,
                sound: *id,
                by_position: *by_position,
                rect_width,
                rect_height,
                position,
            });
            Ok(Value::Null)
        })())
    }
}

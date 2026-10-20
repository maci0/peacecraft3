//! Executed global pause requests; admission does not invent a fresh-game flag.
use super::{Error, Failure, ObjectHost};
use pcx_script::Value;

const PAUSE: &str = "PauseGame";

impl ObjectHost {
    pub fn with_game_pause(mut self) -> Result<Self, Error> {
        if self.game_pause.is_some() {
            return Err(self
                .inner
                .error(PAUSE, Failure::ClockState("game pause already admitted")));
        }
        if self.inner.program.natives().get(PAUSE).is_none()
            && self.inner.program.natives().ambiguous(PAUSE).is_none()
        {
            return Ok(self);
        }
        Self::contract_in(
            &self.inner,
            PAUSE,
            &["boolean"],
            "nothing",
            false,
            None,
            false,
        )?;
        let source = &self
            .inner
            .program
            .natives()
            .get(PAUSE)
            .expect("checked native")
            .source;
        if self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .is_none_or(|n| &n.source != source)
        {
            return Err(self.inner.error(PAUSE, Failure::Source));
        }
        self.game_pause = Some(None);
        Ok(self)
    }

    pub fn game_pause_admitted(&self) -> bool {
        self.game_pause.is_some()
    }

    /// Last executed script request; `None` is not an assumed unpaused value.
    pub fn game_paused(&self) -> Option<bool> {
        self.game_pause.flatten()
    }

    pub(super) fn game_pause_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != PAUSE || self.game_pause.is_none() {
            return None;
        }
        let [Value::Bool(paused)] = args else {
            return Some(Err(self
                .inner
                .error(PAUSE, Failure::ClockState("PauseGame expects one boolean"))));
        };
        self.game_pause = Some(Some(*paused));
        // The current thread must finish its subsequent UI/dialog calls.
        Some(Ok(Value::Null))
    }
}

#[cfg(test)]
#[path = "game_pause_tests.rs"]
mod tests;

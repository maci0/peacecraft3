//! Executed `EndGame` requests. The native records the request and returns to its
//! caller, which keeps running; the consumer decides when the session ends.
//! Score-screen presentation is not implemented: the request carries the flag only.
use super::{Error, Failure, ObjectHost};
use pcx_script::Value;

const END_GAME: &str = "EndGame";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameExitRequest {
    /// The script's `doScoreScreen` argument.
    pub score_screen: bool,
}

pub(super) struct GameExits {
    max_pending: usize,
    requests: Vec<GameExitRequest>,
}

impl ObjectHost {
    /// Validates `EndGame` against the selected source. Missing declarations stay unsupported.
    pub fn with_game_exit(mut self, max_pending: usize) -> Result<Self, Error> {
        if self.game_exit.is_some() {
            return Err(self
                .inner
                .error(END_GAME, Failure::ClockState("game exit already admitted")));
        }
        std::alloc::Layout::array::<GameExitRequest>(max_pending)
            .map_err(|_| self.inner.error(END_GAME, Failure::Capacity))?;
        if self.inner.program.natives().get(END_GAME).is_none()
            && self.inner.program.natives().ambiguous(END_GAME).is_none()
        {
            return Ok(self);
        }
        Self::contract_in(
            &self.inner,
            END_GAME,
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
            .get(END_GAME)
            .expect("checked native")
            .source;
        if self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .is_none_or(|n| &n.source != source)
        {
            return Err(self.inner.error(END_GAME, Failure::Source));
        }
        self.game_exit = Some(GameExits {
            max_pending,
            requests: Vec::new(),
        });
        Ok(self)
    }

    pub fn game_exit_admitted(&self) -> bool {
        self.game_exit.is_some()
    }

    /// Requests recorded since the last take, in call order.
    pub fn game_exit_requests(&self) -> &[GameExitRequest] {
        self.game_exit
            .as_ref()
            .map_or(&[], |exits| exits.requests.as_slice())
    }

    pub fn take_game_exit_requests(&mut self) -> Vec<GameExitRequest> {
        self.game_exit
            .as_mut()
            .map_or_else(Vec::new, |exits| std::mem::take(&mut exits.requests))
    }

    pub(super) fn game_exit_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != END_GAME || self.game_exit.is_none() {
            return None;
        }
        let [Value::Bool(score_screen)] = args else {
            return Some(Err(self.inner.error(
                END_GAME,
                Failure::ClockState("EndGame expects one boolean"),
            )));
        };
        let exits = self.game_exit.as_mut().expect("admitted game exit");
        if exits.requests.len() >= exits.max_pending {
            return Some(Err(self
                .inner
                .error(END_GAME, Failure::Budget("game exit requests"))));
        }
        if exits.requests.try_reserve(1).is_err() {
            return Some(Err(self.inner.error(END_GAME, Failure::Capacity)));
        }
        exits.requests.push(GameExitRequest {
            score_screen: *score_screen,
        });
        // The calling thread continues after EndGame, as in the original native.
        Some(Ok(Value::Null))
    }
}

#[cfg(test)]
#[path = "game_exit_tests.rs"]
mod tests;

//! The map-to-AI command queue behind `CommandAI`, `CommandsWaiting`, `GetLastCommand`,
//! `GetLastData` and `PopLastCommand`.
//!
//! `CommandAI(player, command, data)` appends a pair to that player's queue; the AI
//! script polls `CommandsWaiting` and reads the pair with the two getters, then pops it
//! (`WaitForSignal` in the selected `common.ai`). The count, getters and pop follow that
//! use. Not established: which pair "last" names when several are pending, so a read with
//! more than one pending pair refuses instead of choosing.
use super::{Error, Failure, ObjectHost};
use std::collections::VecDeque;

/// Pending pairs per player; a larger backlog refuses.
const MAX_PENDING: usize = 64;

impl ObjectHost {
    pub(crate) fn push_ai_command(
        &mut self,
        player: i32,
        command: i32,
        data: i32,
    ) -> Result<(), Error> {
        let queue = self.ai_commands.entry(player).or_default();
        if queue.len() >= MAX_PENDING {
            return Err(self.inner.error(
                "CommandAI",
                Failure::ConfigState("AI command backlog is full"),
            ));
        }
        queue.push_back((command, data));
        Ok(())
    }

    pub(crate) fn ai_commands_waiting(&self, player: i32) -> usize {
        self.ai_commands.get(&player).map_or(0, VecDeque::len)
    }

    /// The pending pair, refused when none or several are pending.
    pub(crate) fn ai_last_command(&self, player: i32, native: &str) -> Result<(i32, i32), Error> {
        let queue = self.ai_commands.get(&player);
        match queue.map(|q| (q.len(), q.front())) {
            Some((1, Some(pair))) => Ok(*pair),
            Some((0, _)) | None => Err(self
                .inner
                .error(native, Failure::ConfigState("no AI command is pending"))),
            Some(_) => Err(self.inner.error(
                native,
                Failure::ConfigState(
                    "several AI commands are pending and which one is last is not established",
                ),
            )),
        }
    }

    pub(crate) fn pop_ai_command(&mut self, player: i32) -> Result<(), Error> {
        self.ai_last_command(player, "PopLastCommand")?;
        self.ai_commands.entry(player).or_default().clear();
        Ok(())
    }
}

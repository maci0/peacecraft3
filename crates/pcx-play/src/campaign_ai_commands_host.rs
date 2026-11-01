//! The map-to-AI command queue behind `CommandAI`, `CommandsWaiting`, `GetLastCommand`,
//! `GetLastData` and `PopLastCommand`.
//!
//! `CommandAI(player, command, data)` pushes a pair. The AI script polls
//! `CommandsWaiting`, reads that pair with the two getters, then pops it
//! (`WaitForSignal` in the selected `common.ai`). "Last" is the newest pair,
//! a stack. That reading of `PopLast` is a recreation, not a decoded native.
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

    /// The newest pending pair. An empty queue refuses.
    pub(crate) fn ai_last_command(&self, player: i32, native: &str) -> Result<(i32, i32), Error> {
        self.ai_commands
            .get(&player)
            .and_then(|queue| queue.back().copied())
            .ok_or_else(|| {
                self.inner
                    .error(native, Failure::ConfigState("no AI command is pending"))
            })
    }

    pub(crate) fn pop_ai_command(&mut self, player: i32) -> Result<(), Error> {
        self.ai_last_command(player, "PopLastCommand")?;
        self.ai_commands
            .get_mut(&player)
            .expect("the read found a pair")
            .pop_back();
        Ok(())
    }
}

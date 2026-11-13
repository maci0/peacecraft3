//! A map listener runs in the map VM while the AI native return remains owned.
use super::*;

impl<W: AiWorld> Runtime<W> {
    pub(crate) fn take_map_call(&mut self, player: i32) -> Option<AiMapCall> {
        self.active.get_mut(&player)?.host().pending_map.take()
    }

    pub(crate) fn resume_map_call(
        &mut self,
        world: &mut Option<W>,
        player: i32,
        value: Result<Value, String>,
    ) -> Result<(), String> {
        if self.pending_retirement.contains(&player) || !self.active.contains_key(&player) {
            return Err("campaign AI callback context is not running".into());
        }
        let borrowed = world.take().ok_or("campaign AI world is already lent")?;
        let mut vm = self.active.remove(&player).expect("checked AI player");
        vm.host().world = Some(borrowed);
        let result = vm
            .resume_checkpoint(value)
            .map(|_| ())
            .map_err(|error| script_error(&mut vm, &error));
        *world = vm.host().world.take();
        if result.is_ok() {
            self.active.insert(player, vm);
        } else if let Err(error) = world
            .as_mut()
            .expect("AI returns its canonical world")
            .ai_deactivate_player(player)
        {
            self.active.insert(player, vm);
            self.pending_retirement.insert(player);
            return Err(format!(
                "{}; retirement: {error}",
                result.expect_err("failed AI execution")
            ));
        }
        result.map_err(|error| format!("campaign AI player {player}: {error}"))
    }
}

impl<W: AiWorld> Runtime<W> {
    pub(crate) fn abort_map_call(
        &mut self,
        world: &mut Option<W>,
        player: i32,
        why: &str,
    ) -> Result<(), String> {
        if self.pending_retirement.contains(&player) || !self.active.contains_key(&player) {
            return Ok(());
        }
        let borrowed = world.take().ok_or("campaign AI world is already lent")?;
        let mut vm = self.active.remove(&player).expect("checked AI player");
        vm.host().world = Some(borrowed);
        let _ = vm.resume_checkpoint(Err(why.to_owned()));
        *world = vm.host().world.take();
        if let Err(error) = world
            .as_mut()
            .expect("AI returns its canonical world")
            .ai_deactivate_player(player)
        {
            self.active.insert(player, vm);
            self.pending_retirement.insert(player);
            return Err(error);
        }
        Ok(())
    }
}

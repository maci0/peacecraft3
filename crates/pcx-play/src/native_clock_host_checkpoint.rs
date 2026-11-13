//! Source policy is rebound in the candidate, never copied into the save.
use super::{ObjectHost, State};
use pcx_script::checkpoint::{Reader, Writer};

impl ObjectHost {
    pub fn write_native_clock_checkpoint(&self, out: &mut Writer) -> Result<(), String> {
        out.boolean(self.native_game_clock.is_some())?;
        if let Some(clock) = &self.native_game_clock {
            out.raw(
                self.numeric
                    .as_ref()
                    .ok_or("checkpoint clock numeric source missing")?
                    .image_sha256(),
            )?;
            clock.write_checkpoint(out)?;
        }
        Ok(())
    }
    /// Restore only into the unpublished whole-world candidate; dependent tickets are decoded afterwards.
    pub fn restore_native_clock_checkpoint(
        &mut self,
        input: &mut Reader<'_>,
    ) -> Result<(), String> {
        if input.boolean()? != self.native_game_clock.is_some() {
            return Err("checkpoint clock admission differs".into());
        }
        if let Some(clock) = &self.native_game_clock {
            if input.raw(32)?
                != self
                    .numeric
                    .as_ref()
                    .ok_or("checkpoint clock numeric source missing")?
                    .image_sha256()
            {
                return Err("checkpoint clock image differs".into());
            }
            let next = State::read_checkpoint(clock.rules().clone(), input)?;
            self.native_game_clock = Some(next);
        }
        Ok(())
    }
}

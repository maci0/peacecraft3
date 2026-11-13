use super::Input;
use pcx_script::checkpoint::{Reader, Writer};

impl Input {
    pub(crate) fn write_checkpoint(&self, out: &mut Writer) -> Result<(), String> {
        out.raw(&self.image_sha256)?;
        out.list(self.results.len())?;
        for &(player, result) in &self.results {
            out.i32(player)?;
            out.i32(result)?;
        }
        Ok(())
    }

    pub(crate) fn read_checkpoint(
        &self,
        input: &mut Reader<'_>,
        player_valid: &mut impl FnMut(i32) -> Result<(), String>,
    ) -> Result<Self, String> {
        if input.raw(32)? != self.image_sha256 {
            return Err("checkpoint future AI difficulty source differs".into());
        }
        let count = input.list()?;
        if count > super::super::MAX_PLAYERS {
            return Err("checkpoint AI difficulty input capacity".into());
        }
        input.owned(
            count
                .checked_mul(std::mem::size_of::<(i32, i32)>())
                .ok_or("checkpoint AI difficulty allocation")?,
        )?;
        let mut results = Vec::new();
        results
            .try_reserve_exact(count)
            .map_err(|_| "checkpoint AI difficulty allocation")?;
        for _ in 0..count {
            let player = input.i32()?;
            player_valid(player)?;
            if results.iter().any(|&(previous, _)| previous == player) {
                return Err("duplicate checkpoint AI difficulty player".into());
            }
            results.push((player, input.i32()?));
        }
        Ok(Self {
            image_sha256: self.image_sha256,
            results,
        })
    }
}

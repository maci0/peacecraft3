//! Candidate-world random section. No generator is reconstructed by replaying a seed call.
use super::{DrawLog, ObjectHost, Random, State};
use pcx_script::checkpoint::{Reader, Writer};
use std::sync::Arc;

impl ObjectHost {
    pub fn write_random_checkpoint(&self, out: &mut Writer) -> Result<(), String> {
        out.boolean(self.random.is_some())?;
        let Some(Random {
            policy,
            max_secondary,
            state,
            order,
        }) = &self.random
        else {
            return self.write_unit_random_checkpoint(out);
        };
        out.raw(&policy.key)?;
        out.u64(u64::try_from(*max_secondary).map_err(|_| "random budget overflow")?)?;
        out.boolean(state.is_some())?;
        if let Some(state) = state {
            state.write_checkpoint(out, policy)?;
        }
        order.write_checkpoint(out, policy.secondary_count)?;
        self.write_unit_random_checkpoint(out)
    }

    /// Apply only to the unpublished candidate world. Failure leaves this section unchanged.
    pub fn restore_random_checkpoint(&mut self, input: &mut Reader<'_>) -> Result<(), String> {
        let admitted = input.boolean()?;
        if admitted != self.random.is_some() {
            return Err("checkpoint random admission differs from its candidate world".into());
        }
        let Some(current) = &self.random else {
            let prepared = self.prepare_unit_random_checkpoint(input)?;
            self.commit_unit_random_checkpoint(prepared);
            return Ok(());
        };
        if input.raw(current.policy.key.len())? != current.policy.key {
            return Err("checkpoint random policy differs from its candidate source".into());
        }
        let max_secondary =
            usize::try_from(input.u64()?).map_err(|_| "checkpoint random budget overflow")?;
        if max_secondary > current.max_secondary || max_secondary < current.policy.secondary_count {
            return Err("checkpoint random allocation exceeds its candidate policy".into());
        }
        let state = if input.boolean()? {
            Some(State::read_checkpoint(input, &current.policy)?)
        } else {
            None
        };
        let order = DrawLog::read_checkpoint(input, current.policy.secondary_count)?;
        if state.is_none()
            && order
                .records()
                .iter()
                .any(|record| record.primary_word.is_some())
        {
            return Err("checkpoint random caller words require initialized streams".into());
        }
        let prepared_unit = self.prepare_unit_random_checkpoint(input)?;
        self.random = Some(Random {
            policy: Arc::clone(&current.policy),
            max_secondary,
            state,
            order,
        });
        self.commit_unit_random_checkpoint(prepared_unit);
        Ok(())
    }
}

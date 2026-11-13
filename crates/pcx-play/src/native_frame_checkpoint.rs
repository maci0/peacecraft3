//! Real queue progress is independent of the completed game cursor.
use super::*;
use crate::native_game_clock::checkpoint::{
    pending_in, pending_out, point_in, point_out, selected_in, selected_out, validate_pending,
    validate_point,
};
use pcx_script::checkpoint::{Reader, Writer};

impl Queue {
    pub(in crate::native_game_clock) fn write_checkpoint(
        &self,
        out: &mut Writer,
    ) -> Result<(), String> {
        point_out(out, self.point)?;
        pending_out(out, self.pending)?;
        selected_out(out, self.selected)?;
        for value in [self.remaining, self.amount] {
            out.boolean(value.is_some())?;
            if let Some(value) = value {
                out.u32(value)?;
            }
        }
        Ok(())
    }
    pub(in crate::native_game_clock) fn read_checkpoint(
        input: &mut Reader<'_>,
    ) -> Result<Self, String> {
        Ok(Self {
            point: point_in(input)?,
            pending: pending_in(input)?,
            selected: selected_in(input)?,
            remaining: if input.boolean()? {
                Some(input.u32()?)
            } else {
                None
            },
            amount: if input.boolean()? {
                Some(input.u32()?)
            } else {
                None
            },
        })
    }
    pub(in crate::native_game_clock) fn validate_checkpoint(
        &self,
        rules: &ClockRules,
        real_frame: Option<u32>,
    ) -> Result<(), String> {
        validate_point(rules, self.point)?;
        if self.pending.is_some() != self.amount.is_some() {
            return Err("checkpoint real chunk is incomplete".into());
        }
        if let Some(remaining) = self.remaining {
            let original = real_frame.ok_or("checkpoint real duration has no owning frame")?;
            for value in [remaining, original] {
                real::from_f32(f32::from_bits(value)).map_err(|e| e.to_string())?;
            }
            if f32::from_bits(remaining) <= 0. || !real::below_or_equal(remaining, original) {
                return Err("checkpoint real duration exceeds its frame".into());
            }
            if let Some(amount) = self.amount {
                let expected = if real::below_or_equal(remaining, rules.frame.chunk) {
                    remaining
                } else {
                    rules.frame.chunk
                };
                if amount != expected {
                    return Err("checkpoint real chunk differs from source split".into());
                }
                validate_pending(rules, self.point, self.pending, amount)?;
            }
        } else if self.amount.is_some() {
            return Err("checkpoint real chunk has no duration".into());
        }
        Ok(())
    }
    pub(in crate::native_game_clock) fn checkpoint_idle(&self) -> bool {
        self.pending.is_none()
            && self.selected.is_none()
            && self.remaining.is_none()
            && self.amount.is_none()
    }
}

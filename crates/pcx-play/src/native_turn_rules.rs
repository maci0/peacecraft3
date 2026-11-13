//! Fresh local turn grants; replay and external controller overrides are not admitted.
use super::{call, frame, shape, word};
use std::{sync::Arc, time::Duration};

#[derive(Clone, Debug)]
pub struct Rules {
    pub quantum_ms: u32,
    pub credit_cap: u32,
    pub speed_steps: Arc<[u32]>,
    pub scale: u32,
    pub modifier: u32,
    pub elapsed_numerator: u32,
    pub elapsed_denominator: u32,
    pub initial_carry: u32,
}

impl Rules {
    pub(super) fn read(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        zero_filled: impl Fn(u32, usize) -> bool,
        frame: &frame::Rules,
        budget: usize,
    ) -> Result<Self, String> {
        let gate = read(0x5af1c2, 7, true)?;
        shape(&gate, 0, &[0x83, 0xbf, 0x18, 0x26, 0, 0])?;
        let quantum_ms = u32::from(gate[6]);
        let consume = read(0x5af226, 16, true)?;
        shape(&consume, 0, &[0x83, 0xc0])?;
        shape(&consume, 9, &[0x83, 0x87, 0x18, 0x26, 0, 0])?;
        if i32::from(consume[2] as i8) != -(quantum_ms as i32) || consume[2] != consume[15] {
            return Err("native turn quantum consumers differ".into());
        }
        let cap = read(0x5af183, 5, true)?;
        shape(&cap, 0, &[0xb9])?;
        let credit_cap = word(&cap, 1)?;
        let setter = read(0x5ae777, 0x21, true)?;
        shape(&setter, 0, &[0xb9])?;
        shape(&setter, 5, &[0x39, 0x4d, 8, 0x0f, 0x42, 0x4d, 8])?;
        shape(
            &setter,
            0x14,
            &[0x89, 0x82, 0x70, 0x26, 0, 0, 0x8b, 4, 0x85],
        )?;
        let count = usize::try_from(word(&setter, 1)?)
            .map_err(|_| "native speed count")?
            .checked_add(1)
            .ok_or("native speed count overflow")?;
        let length = count
            .checked_mul(4)
            .filter(|len| *len <= budget)
            .ok_or("native speed table input budget")?;
        let table = word(&setter, 0x1d)?;
        let raw = read(table, length, false)?;
        let mut speeds = Vec::new();
        speeds
            .try_reserve_exact(count)
            .map_err(|_| "native speed table allocation")?;
        for i in 0..count {
            speeds.push(word(&raw, i * 4)?);
        }
        let n = read(0x5af2b9, 0x73, true)?;
        shape(&n, 0, &[0x8b, 4, 0x9d])?;
        shape(
            &n,
            7,
            &[
                0x0f, 0xaf, 0xc1, 0x66, 0x0f, 0x6e, 0xc0, 0xf3, 0x0f, 0xe6, 0xc0,
            ],
        )?;
        shape(&n, 0x22, &[0xf3, 0x0f, 0x59, 0x87])?;
        shape(&n, 0x2a, &[0xf3, 0x0f, 0x58, 5])?;
        call(&n, 0x5af2b9, 0x37, 0xa56bf0)?;
        shape(&n, 0x67, &[0xf3, 0x0f, 0x5c, 0xc8, 0xf3, 0x0f, 0x11, 0x0d])?;
        if word(&n, 3)? != table
            || word(&n, 0x2e)? != word(&n, 0x6f)?
            || !zero_filled(word(&n, 0x2e)?, 4)
        {
            return Err("native turn table or initial carry binding differs".into());
        }
        let correction = read(word(&n, 0x1a)?, 16, false)?;
        for (i, bits) in frame.unsigned_correction.iter().enumerate() {
            if correction.get(i * 8..i * 8 + 8) != Some(bits.to_le_bytes().as_slice()) {
                return Err("native turn unsigned conversion differs".into());
            }
        }
        let scale = read(0x5a1b7a, 10, true)?;
        shape(&scale, 0, &[0xc7, 0x83])?;
        if word(&scale, 2)? != word(&n, 0x26)? {
            return Err("native turn scale field differs".into());
        }
        let numerator = read(0x5a1b8e, 10, true)?;
        let denominator = read(0x5a1ba3, 10, true)?;
        shape(&numerator, 0, &[0xc7, 0x83, 0x80, 0x26, 0, 0])?;
        shape(&denominator, 0, &[0xc7, 0x83, 0x84, 0x26, 0, 0])?;
        let fallback = read(0x5af2b4, 5, true)?;
        shape(&fallback, 0, &[0xb9])?;
        let modifier_initial = read(0x4063f7, 10, true)?;
        shape(&modifier_initial, 0, &[0xc7, 5])?;
        let modifier_read = read(0x5237a0, 21, true)?;
        shape(&modifier_read, 5, &[0x8b, 0x0d])?;
        if word(&modifier_initial, 2)? != word(&modifier_read, 7)?
            || word(&modifier_initial, 6)? != 0
        {
            return Err("native fresh modifier is unsupported".into());
        }
        let result = Self {
            quantum_ms,
            credit_cap,
            speed_steps: speeds.into(),
            scale: word(&scale, 6)?,
            modifier: word(&fallback, 1)?,
            elapsed_numerator: word(&numerator, 6)?,
            elapsed_denominator: word(&denominator, 6)?,
            initial_carry: 0,
        }; // The PE loader zero-fills the bound carry storage.
        result.validate()?;
        Ok(result)
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.quantum_ms == 0
            || self.credit_cap < self.quantum_ms
            || self.speed_steps.is_empty()
            || !f32::from_bits(self.scale).is_finite()
            || f32::from_bits(self.scale) <= 0.
            || self.elapsed_denominator == 0
            || !f32::from_bits(self.initial_carry).is_finite()
        {
            return Err("native fresh turn domain invalid".into());
        }
        Ok(())
    }
    pub(super) fn grant(
        &self,
        speed: usize,
        carry: u32,
        frame: &frame::Rules,
    ) -> Result<(u32, u32), String> {
        let count = self
            .speed_steps
            .get(speed)
            .ok_or("native speed is outside the selected table")?
            .wrapping_mul(self.modifier);
        let value = frame.unsigned(count) * f32::from_bits(self.scale);
        let value = value + f32::from_bits(carry);
        if !value.is_finite() || value < 0. || f64::from(value) > f64::from(u32::MAX) {
            return Err("native turn conversion is outside the qualified domain".into());
        }
        let count = value as u32;
        Ok((count, (value - frame.unsigned(count)).to_bits()))
    }
    pub(super) fn duration(
        &self,
        steps: u32,
        numerator: u32,
        denominator: u32,
    ) -> Result<Duration, String> {
        let nanos = u128::from(steps)
            .checked_mul(u128::from(numerator))
            .and_then(|n| n.checked_mul(1_000_000_000))
            .ok_or("native grant duration overflow")?;
        if denominator == 0 || nanos % u128::from(denominator) != 0 {
            return Err("native grant does not fit duration units".into());
        }
        Ok(Duration::from_nanos(
            u64::try_from(nanos / u128::from(denominator))
                .map_err(|_| "native grant duration overflow")?,
        ))
    }
}

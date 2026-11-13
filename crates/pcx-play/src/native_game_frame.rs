//! Independently supplied real-clock progress, after a frame's game work.
use super::{Pending, Point, QueueError, Rules as ClockRules, Segment, call, real, shape, word};
use crate::script_numeric::NumericPolicy;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameInput {
    pub game_elapsed: Duration,
    pub real_elapsed_ms: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub unsigned_correction: [u64; 2],
    pub scale: u32,
    pub chunk: u32,
}

impl Rules {
    pub(super) fn read(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        numeric: &NumericPolicy,
    ) -> Result<Self, String> {
        let producer = read(0x5af4a3, 0x89, true)?;
        shape(&producer, 0, &[0x8b, 0x45, 0xe0, 0x85, 0xc0, 0x74])?;
        shape(&producer, 0xe, &[0xc7, 0x45, 0xd0])?;
        if word(&producer, 0x11)? != 0x40090094 {
            return Err("native asynchronous clock event differs".into());
        }
        shape(&producer, 0x23, &[0x66, 0x0f, 0x6e, 0xc0])?;
        shape(&producer, 0x2a, &[0xf3, 0x0f, 0xe6, 0xc0, 0xc1, 0xe8, 0x1f])?;
        shape(&producer, 0x4d, &[0xf2, 0x0f, 0x58, 4, 0xc5])?;
        shape(
            &producer,
            0x58,
            &[0x66, 0x0f, 0x5a, 0xc0, 0xf3, 0x0f, 0x59, 5],
        )?;
        shape(&producer, 0x64, &[0xf3, 0x0f, 0x11, 0x45, 0xdc])?;
        let table = read(word(&producer, 0x52)?, 16, false)?;
        let unsigned_correction = [0, 8].map(|at| {
            table
                .get(at..at + 8)
                .and_then(|b| b.try_into().ok())
                .map(u64::from_le_bytes)
                .ok_or_else(|| "native unsigned clock conversion table truncated".to_owned())
        });
        let unsigned_correction = [
            unsigned_correction[0].clone()?,
            unsigned_correction[1].clone()?,
        ];
        let scale = word(&read(word(&producer, 0x60)?, 4, false)?, 0)?;
        let initializer = read(0x40d060, 19, true)?;
        let (chunk_address, chunk) = numeric
            .integer_initializer(&initializer, 0x40d060)
            .ok_or("native real-clock chunk initializer differs")?;
        let advance = read(0x7f4820, 0xe2, true)?;
        shape(&advance, 0x22, &[0x83, 0x7e, 0x40, 0])?;
        shape(&advance, 0x42, &[0xff, 0x76, 0x48, 0x68])?;
        for at in [0x3c, 0x46, 0x59, 0x6e, 0x78, 0x99] {
            if word(&advance, at)? != chunk_address {
                return Err("native real-clock chunk consumers differ".into());
            }
        }
        call(&advance, 0x7f4820, 0x4a, 0x7fac80)?;
        call(&advance, 0x7f4820, 0x84, 0x9a00b0)?;
        call(&advance, 0x7f4820, 0xaf, 0x7fac80)?;
        let forward = read(0x5ea690, 0x50, true)?;
        shape(&forward, 0x28, &[0x83, 0xbe, 0x98, 2, 0, 0, 0])?;
        call(&forward, 0x5ea690, 0x3f, 0x7f4820)?;
        let game_queue = read(0x7f73cc, 28, true)?;
        let real_queue = read(0x7f7419, 15, true)?;
        shape(&real_queue, 0, &[0x68])?;
        call(&real_queue, 0x7f7419, 7, 0x7f3390)?;
        shape(&real_queue, 12, &[0x89, 0x47, 0x48])?;
        if word(&real_queue, 1)? != word(&game_queue, 1)? {
            return Err("native game and real queues have different cycle initializers".into());
        }
        Ok(Self {
            unsigned_correction,
            scale,
            chunk: chunk.to_bits(),
        })
    }

    pub(super) fn validate(&self, cycle: f32) -> Result<(), String> {
        for bits in self.unsigned_correction {
            if !f64::from_bits(bits).is_finite() {
                return Err("native real-clock unsigned conversion is not finite".into());
            }
        }
        for bits in [self.scale, self.chunk] {
            let value = f32::from_bits(bits);
            real::from_f32(value).map_err(|e| e.to_string())?;
            if value <= 0. {
                return Err("native real-clock operand must be positive".into());
            }
        }
        if f32::from_bits(self.chunk) >= cycle {
            return Err("native real-clock chunk must precede its cycle limit".into());
        }
        Ok(())
    }

    pub(super) fn delta(&self, milliseconds: u32) -> Result<Option<u32>, String> {
        if milliseconds == 0 {
            return Ok(None);
        }
        // The producer uses SSE conversion and multiplication, before queue soft arithmetic.
        let converted = self.unsigned(milliseconds);
        let delta = converted * f32::from_bits(self.scale);
        let word = real::from_f32(delta).map_err(|e| e.to_string())?;
        if delta <= 0. {
            return Err("native real-clock conversion did not progress".into());
        }
        Ok(Some(word))
    }
    pub(super) fn unsigned(&self, value: u32) -> f32 {
        (f64::from(value as i32) + f64::from_bits(self.unsigned_correction[(value >> 31) as usize]))
            as f32
    }
}

#[derive(Clone, Debug)]
pub(super) struct Queue {
    pub point: Point,
    pub pending: Option<Pending>,
    pub selected: Option<Point>,
    remaining: Option<u32>,
    amount: Option<u32>,
}

impl Queue {
    pub fn new(origin: u32) -> Self {
        Self {
            point: Point {
                cycle: 0,
                local: origin,
            },
            pending: None,
            selected: None,
            remaining: None,
            amount: None,
        }
    }
    pub fn point(&self) -> Point {
        self.selected
            .unwrap_or_else(|| self.pending.map_or(self.point, |p| p.segment.point))
    }
    pub fn begin(&mut self, delta: Option<u32>) -> Result<(), String> {
        if self.remaining.is_some() || self.pending.is_some() || self.selected.is_some() {
            return Err("native real-clock frame already pending".into());
        }
        self.remaining = delta;
        Ok(())
    }
    pub fn next(
        &mut self,
        rules: &ClockRules,
        work: &mut usize,
    ) -> Result<Option<Segment>, QueueError> {
        if let Some(pending) = self.pending {
            return Ok(Some(pending.segment));
        }
        let Some(remaining) = self.remaining else {
            return Ok(None);
        };
        if *work == 0 {
            return Err(QueueError::Budget("native real-clock chunk"));
        }
        let amount = if real::below_or_equal(remaining, rules.frame.chunk) {
            remaining
        } else {
            rules.frame.chunk
        };
        let pending = super::prepare_drain(rules, self.point, amount)?;
        self.pending = Some(pending);
        self.amount = Some(amount);
        *work -= 1;
        Ok(Some(pending.segment))
    }
    pub fn finish(&mut self, rules: &ClockRules, segment: Segment) -> Result<(), String> {
        let mut candidate = self.clone();
        let done = super::finish_drain(
            &mut candidate.point,
            &mut candidate.pending,
            candidate.selected,
            segment,
        )?;
        if done {
            let remaining = candidate
                .remaining
                .ok_or("native real-clock duration missing")?;
            let amount = candidate
                .amount
                .take()
                .ok_or("native real-clock chunk missing")?;
            candidate.remaining = if real::below_or_equal(remaining, amount) {
                None
            } else {
                Some(real::sub(remaining, amount).map_err(|e| e.to_string())?)
            };
        }
        if f32::from_bits(candidate.point.local) > rules.simple.cycle {
            return Err("native real-clock completed point is outside the cycle".into());
        }
        *self = candidate;
        Ok(())
    }
}

//! Native timer query readpoints; delivery deadlines remain caller-owned.
use super::{Error, NumericPolicy, Problem, at, call, real, word};
use crate::{
    camera_margins::SourceWindow,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct Binding {
    pub image_sha256: Sha256Digest,
    pub segment_initializer: SourceWindow,
    pub zero_initializer: SourceWindow,
    pub constructor: SourceWindow,
    pub start: SourceWindow,
    pub schedule: SourceWindow,
    pub remainder: SourceWindow,
    pub remaining: SourceWindow,
    pub full_timeout: SourceWindow,
    pub active_elapsed: SourceWindow,
    pub inactive_elapsed: SourceWindow,
}

#[derive(Debug)]
pub(crate) struct Rules {
    image: Sha256Digest,
    segment: u32,
    minimum: u32,
    zero: u32,
}

/// A separate query epoch preserves the source's arithmetic order and readpoints.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Epoch {
    image: Sha256Digest,
    period: u32,
    whole: u16,
    remainder: u32,
    first: u32,
    pub(crate) expiry: f32,
}

impl Epoch {
    pub(crate) fn segmented(self) -> bool {
        self.whole > 0
    }
}

impl Rules {
    pub(super) fn read(
        image: &[u8],
        b: &Binding,
        n: &NumericPolicy,
        budget: RuleInputBudget,
    ) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: n.origin().source.clone(),
            problem,
        };
        let windows = [
            &b.segment_initializer,
            &b.zero_initializer,
            &b.constructor,
            &b.start,
            &b.schedule,
            &b.remainder,
            &b.remaining,
            &b.full_timeout,
            &b.active_elapsed,
            &b.inactive_elapsed,
        ];
        if image.len() > budget.image_bytes
            || windows.iter().map(|w| u64::from(w.len)).sum::<u64>() > budget.profile_bytes as u64
        {
            return Err(fail(Problem::Budget));
        }
        if b.image_sha256 != *n.image_sha256() || sha256(image) != b.image_sha256 {
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        let open = |w: &SourceWindow| pe.window(w, true).map_err(|e| fail(Problem::Image(e)));
        let (segment_va, segment) = n
            .integer_initializer(open(&b.segment_initializer)?, b.segment_initializer.va)
            .ok_or_else(|| fail(Problem::Shape))?;
        let (zero_va, zero) = n
            .integer_initializer(open(&b.zero_initializer)?, b.zero_initializer.va)
            .ok_or_else(|| fail(Problem::Shape))?;
        let start = open(&b.start)?;
        let schedule = open(&b.schedule)?;
        let rem = open(&b.remainder)?;
        let remain = open(&b.remaining)?;
        let full = open(&b.full_timeout)?;
        let elapsed = open(&b.active_elapsed)?;
        let inactive = open(&b.inactive_elapsed)?;
        let constructor = open(&b.constructor)?;
        let minimum_va = word(schedule, 0xb).ok_or_else(|| fail(Problem::Shape))?;
        let minimum = word(
            pe.read_from(minimum_va, false)
                .map_err(|e| fail(Problem::Image(e)))?,
            0,
        )
        .ok_or_else(|| fail(Problem::Shape))?;
        let calls = |bytes, base, roles: &[(usize, u32)]| {
            roles
                .iter()
                .all(|&(offset, target)| call(bytes, base, offset) == Some(target))
        };
        if segment < 1.0
            || segment >= i32::MAX as f32
            || zero != 0.0
            || real::from_f32(f32::from_bits(minimum)).is_err()
            || f32::from_bits(minimum) <= 0.0
            || f32::from_bits(minimum) >= segment
            || ![0x89, 0x9c, 0xd9]
                .iter()
                .all(|&offset| word(start, offset) == Some(segment_va))
            || !at(start, 0x99, &[0xf7, 0xf9])
            || !at(start, 0xa3, &[0x66, 0x89, 0x43, 0x14])
            || call(start, b.start.va, 0xab) != Some(b.remainder.va)
            || ![0xdf, 0xf3]
                .iter()
                .all(|&offset| call(start, b.start.va, offset) == Some(b.schedule.va))
            || !at(schedule, 7, &[0xf3, 0x0f, 0x10, 5])
            || word(schedule, 0x1e) != Some(minimum_va)
            || call(schedule, b.schedule.va, 0x37) != Some(n.binding.math_add.va)
            || word(rem, 0x33) != Some(n.binding.reciprocal_table.va)
            || word(rem, 0x3e) != n.binding.reciprocal_table.va.checked_add(4)
            || !calls(
                rem,
                b.remainder.va,
                &[
                    (0x79, n.binding.math_mul.va),
                    (0xb2, n.binding.math_sub.va),
                    (0xcf, n.binding.math_mul.va),
                ],
            )
            || word(full, 0x6e) != Some(segment_va)
            || !calls(
                full,
                b.full_timeout.va,
                &[
                    (0x77, n.binding.math_i32_to_real.va),
                    (0x84, n.binding.math_mul.va),
                    (0x91, n.binding.math_add.va),
                ],
            )
            || word(remain, 0x66) != Some(segment_va)
            || word(remain, 0x54) != Some(zero_va)
            || !calls(
                remain,
                b.remaining.va,
                &[
                    (0x6f, n.binding.math_i32_to_real.va),
                    (0x7c, n.binding.math_mul.va),
                    (0x8a, n.binding.math_add.va),
                    (0xa9, n.binding.math_sub.va),
                    (0xba, n.binding.math_add.va),
                    (0xda, n.binding.math_add.va),
                ],
            )
            || !at(elapsed, 0x11, &[0xff, 0x50, 0x18])
            || !at(elapsed, 0x1d, &[0xff, 0x50, 0x10])
            || call(elapsed, b.active_elapsed.va, 0x25) != Some(n.binding.math_sub.va)
            || !at(inactive, 0x43, &[0x8b, 0x46, 0x50])
            || !at(inactive, 0x4c, &[0x8b, 0x46, 0x48])
            || call(inactive, b.inactive_elapsed.va, 0x5e) != Some(n.binding.math_sub.va)
            || word(constructor, 0xa8) != Some(zero_va)
            || word(constructor, 0xb7) != Some(zero_va)
        {
            return Err(fail(Problem::Shape));
        }
        Ok(Self {
            image: b.image_sha256,
            segment: segment.to_bits(),
            minimum,
            zero: zero.to_bits(),
        })
    }

    pub(crate) fn check_epoch(&self, epoch: Epoch) -> bool {
        epoch.image == self.image
    }

    pub(crate) fn zero(&self) -> f32 {
        f32::from_bits(self.zero)
    }

    pub(crate) fn begin(
        &self,
        n: &NumericPolicy,
        now: f32,
        timeout: f32,
        work: usize,
    ) -> Result<Epoch, String> {
        let calculate = || {
            let now = real::from_f32(now)?;
            let t = real::from_f32(timeout)?;
            if timeout < 0.0 || timeout >= i32::MAX as f32 {
                return Err(real::MathError::Unsupported);
            }
            let count = real::truncate(t)? / real::truncate(self.segment)?;
            let whole = u16::try_from(count).map_err(|_| real::MathError::Unsupported)?;
            if usize::from(whole) > work {
                return Err(real::MathError::Unsupported);
            }
            let remainder = n.positive_remainder(t, self.segment)?;
            let period = if whole > 0 {
                self.segment
            } else {
                self.clamp(remainder)
            };
            let first = real::add(now, period)?;
            let mut expiry = first;
            for _ in 1..whole {
                expiry = real::add(expiry, self.segment)?;
            }
            if whole > 0 {
                expiry = real::add(expiry, self.clamp(remainder))?;
            }
            Ok(Epoch {
                image: self.image,
                period,
                whole,
                remainder,
                first,
                expiry: f32::from_bits(expiry),
            })
        };
        calculate().map_err(|e: real::MathError| n.runtime_error(e))
    }

    fn clamp(&self, value: u32) -> u32 {
        if f32::from_bits(value) < f32::from_bits(self.minimum) {
            self.minimum
        } else {
            value
        }
    }

    pub(crate) fn remaining(
        &self,
        n: &NumericPolicy,
        epoch: Epoch,
        now: f32,
    ) -> Result<f32, String> {
        if !self.check_epoch(epoch) {
            return Err(n.runtime_error("timer query image changed during run"));
        }
        let calculate = || {
            let now = real::from_f32(now)?;
            let mut count = epoch.whole;
            let mut at = epoch.first;
            while count > 0 && real::below_or_equal(at, now) {
                count -= 1;
                at = real::add(
                    at,
                    if count > 0 {
                        self.segment
                    } else {
                        self.clamp(epoch.remainder)
                    },
                )?;
            }
            if epoch.whole == 0 {
                return real::sub(at, now);
            }
            let mut sum = self.zero;
            if count > 1 {
                sum = real::add(
                    sum,
                    real::mul(real::from_i32(i32::from(count) - 1)?, self.segment)?,
                )?;
            }
            sum = real::add(sum, real::sub(at, now)?)?;
            if count > 0 {
                sum = real::add(sum, epoch.remainder)?;
            }
            Ok(sum)
        };
        calculate()
            .map(f32::from_bits)
            .map_err(|e: real::MathError| n.runtime_error(e))
    }

    pub(crate) fn elapsed(
        &self,
        n: &NumericPolicy,
        epoch: Epoch,
        remaining: f32,
    ) -> Result<f32, String> {
        let calculate = || {
            let full = if epoch.whole == 0 {
                epoch.period
            } else {
                real::add(
                    real::mul(real::from_i32(i32::from(epoch.whole))?, self.segment)?,
                    epoch.remainder,
                )?
            };
            real::sub(full, real::from_f32(remaining)?)
        };
        calculate()
            .map(f32::from_bits)
            .map_err(|e: real::MathError| n.runtime_error(e))
    }

    pub(crate) fn inactive_elapsed(
        &self,
        n: &NumericPolicy,
        timeout: f32,
        cached: f32,
    ) -> Result<f32, String> {
        real::sub(timeout.to_bits(), cached.to_bits())
            .map(f32::from_bits)
            .map_err(|e| n.runtime_error(e))
    }
}

#[cfg(test)]
#[path = "timer_query_tests.rs"]
pub(crate) mod tests;

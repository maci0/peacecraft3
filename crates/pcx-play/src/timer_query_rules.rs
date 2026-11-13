//! Native timer query readpoints; delivery deadlines remain caller-owned.
use super::{Error, NumericPolicy, Problem, at, call, real, word};
use crate::{
    camera_margins::SourceWindow,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[path = "timer_query_checkpoint.rs"]
mod checkpoint;

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
    pub pause: SourceWindow,
    pub resume: SourceWindow,
    pub rearm: SourceWindow,
    pub stop: SourceWindow,
    pub dispatch: SourceWindow,
    pub expiry_dispatch: SourceWindow,
}

#[derive(Debug)]
pub(crate) struct Rules {
    image: Sha256Digest,
    segment: u32,
    minimum: u32,
    zero: u32,
}

/// A separate query epoch preserves the source's arithmetic order and readpoints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Epoch {
    image: Sha256Digest,
    period: u32,
    whole: u16,
    remaining_count: u16,
    remainder: u32,
    first: u32,
    /// Fallback projection, never the canonical queue entry.
    /// Legacy final projection; sleep epochs retain the first entry and never read this field.
    pub(crate) expiry: f32,
}

impl Epoch {
    pub(crate) fn entry_deadline(self) -> f32 {
        f32::from_bits(self.first)
    }

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
            &b.pause,
            &b.resume,
            &b.rearm,
            &b.stop,
            &b.dispatch,
            &b.expiry_dispatch,
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
        let pause = open(&b.pause)?;
        let resume = open(&b.resume)?;
        let rearm = open(&b.rearm)?;
        let stop = open(&b.stop)?;
        let dispatch = open(&b.dispatch)?;
        let expiry_dispatch = open(&b.expiry_dispatch)?;
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
            || call(pause, b.pause.va, 0x3b) != Some(b.stop.va)
            || !at(stop, 0x36, &[0x83, 0x66, 0x10, 0xf9])
            || !at(resume, 0x11, &[0x8b, 0x41, 0x50])
            || !at(resume, 0x17, &[0x8b, 0x41, 0x34])
            || !at(resume, 0x1c, &[0xd1, 0xe8, 0x83, 0xe0, 1])
            || call(resume, b.resume.va, 0x2f).is_none()
            || call(resume, b.resume.va, 0x2f) != call(rearm, b.rearm.va, 0x5e)
            || !at(rearm, 0xd, &[0x8b, 0x41, 0x24])
            || !at(rearm, 0x13, &[0x85, 0xc0, 0x74])
            || !at(rearm, 0x17, &[0x8b, 0x46, 0x48])
            || !at(rearm, 0x1b, &[0x8b, 0x7e, 0x34])
            || call(dispatch, b.dispatch.va, 0x29) != Some(b.rearm.va)
            || call(dispatch, b.dispatch.va, 0x22).is_none()
            || call(dispatch, b.dispatch.va, 0x22) != call(dispatch, b.dispatch.va, 0x3b)
            || word(expiry_dispatch, 0x8c) != Some(zero_va)
            || call(expiry_dispatch, b.expiry_dispatch.va, 0xaa) != Some(b.schedule.va)
            || call(expiry_dispatch, b.expiry_dispatch.va, 0xee) != Some(b.stop.va)
            || word(expiry_dispatch, 0x10e) != Some(segment_va)
            || call(expiry_dispatch, b.expiry_dispatch.va, 0x11e) != Some(b.schedule.va)
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

    /// Sleep retains only the current entry; future entries consume work when due.
    pub(crate) fn begin_sleep(
        &self,
        n: &NumericPolicy,
        now: f32,
        delay: f32,
        work: usize,
    ) -> Result<Epoch, String> {
        if work == 0 {
            return Err(n.runtime_error("sleep preparation requires one current entry"));
        }
        real::from_f32(delay).map_err(|e| n.runtime_error(e))?;
        if delay <= -f32::from_bits(self.segment) {
            return Err(n.runtime_error("negative segmented sleep is unqualified"));
        }
        self.begin_entry(
            n,
            now,
            if delay < 0.0 {
                f32::from_bits(self.zero)
            } else {
                delay
            },
        )
    }

    pub(crate) fn begin(
        &self,
        n: &NumericPolicy,
        now: f32,
        timeout: f32,
        work: usize,
    ) -> Result<Epoch, String> {
        let mut epoch = self.begin_entry(n, now, timeout)?;
        if usize::from(epoch.whole) > work {
            return Err(n.runtime_error(real::MathError::Unsupported));
        }
        let project = || {
            let mut expiry = epoch.first;
            for _ in 1..epoch.whole {
                expiry = real::add(expiry, self.segment)?;
            }
            if epoch.whole > 0 {
                expiry = real::add(expiry, self.clamp(epoch.remainder))?;
            }
            Ok(expiry)
        };
        epoch.expiry = f32::from_bits(project().map_err(|e: real::MathError| n.runtime_error(e))?);
        Ok(epoch)
    }

    fn begin_entry(&self, n: &NumericPolicy, now: f32, timeout: f32) -> Result<Epoch, String> {
        let calculate = || {
            let now = real::from_f32(now)?;
            let t = real::from_f32(timeout)?;
            if timeout < 0.0 || timeout >= i32::MAX as f32 {
                return Err(real::MathError::Unsupported);
            }
            let count = real::truncate(t)? / real::truncate(self.segment)?;
            let whole = u16::try_from(count).map_err(|_| real::MathError::Unsupported)?;
            // The remainder uses the fractional reciprocal product, not host `%`.
            let q = real::mul(t, n.reciprocal(self.segment))?;
            let fractional = real::sub(q, real::from_i32(real::truncate(q)?)?)?;
            let mut remainder = real::mul(fractional, self.segment)?;
            if f32::from_bits(remainder) >= f32::from_bits(self.segment) {
                remainder = real::sub(remainder, self.segment)?;
            }
            let period = if whole > 0 {
                self.segment
            } else {
                self.clamp(remainder)
            };
            let first = real::add(now, period)?;
            Ok(Epoch {
                image: self.image,
                period,
                whole,
                remaining_count: whole,
                remainder,
                first,
                expiry: f32::from_bits(first),
            })
        };
        calculate().map_err(|e: real::MathError| n.runtime_error(e))
    }

    /// Consume exactly one due native entry. `true` means the script expiry is due.
    /// The caller drains internal entries before dispatching or rebasing the queue.
    pub(crate) fn advance_entry(
        &self,
        n: &NumericPolicy,
        epoch: &mut Epoch,
    ) -> Result<bool, String> {
        if !self.check_epoch(*epoch) {
            return Err(n.runtime_error("timer query image changed during run"));
        }
        if epoch.remaining_count == 0 {
            return Ok(true);
        }
        let mut next = *epoch;
        next.remaining_count -= 1;
        let delay = if next.remaining_count > 0 {
            self.segment
        } else {
            self.clamp(next.remainder)
        };
        next.first = real::add(next.first, delay).map_err(|e| n.runtime_error(e))?;
        *epoch = next;
        Ok(false)
    }

    /// Rebase only the actual pending entry, after all entries due at the boundary drain.
    /// Keep the authored whole count for full-timeout queries; callers preflight copies.
    pub(crate) fn rebase_epoch(
        &self,
        n: &NumericPolicy,
        epoch: &mut Epoch,
        cycle_limit_bits: u32,
    ) -> Result<(), String> {
        if !self.check_epoch(*epoch) {
            return Err(n.runtime_error("timer query image changed during run"));
        }
        let cycle = f32::from_bits(cycle_limit_bits);
        real::from_f32(cycle).map_err(|e| n.runtime_error(e))?;
        if cycle <= 0.0 {
            return Err(n.runtime_error("invalid native timer cycle"));
        }
        let first = real::sub(epoch.first, cycle_limit_bits).map_err(|e| n.runtime_error(e))?;
        epoch.first = first;
        Ok(())
    }

    /// Preserve ordinary caller deadlines; apply only the qualified residual/minimum edges.
    pub(crate) fn delivery_delay(&self, epoch: Epoch, timeout: f64) -> f64 {
        let minimum = f64::from(f32::from_bits(self.minimum));
        if epoch.whole > 0 && f64::from(f32::from_bits(epoch.remainder)) < minimum {
            f64::from(epoch.whole) * f64::from(f32::from_bits(self.segment)) + minimum
        } else {
            timeout.max(minimum)
        }
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
        let now_bits = real::from_f32(now).map_err(|e| n.runtime_error(e))?;
        let mut projected = epoch;
        while projected.remaining_count > 0 && real::below_or_equal(projected.first, now_bits) {
            self.advance_entry(n, &mut projected)?;
        }
        self.remaining_current(n, projected, now)
    }

    /// Read the actual pending entry/count; canonical queues advance entries explicitly.
    pub(crate) fn remaining_current(
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
            let count = epoch.remaining_count;
            let at = epoch.entry_deadline().to_bits();
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

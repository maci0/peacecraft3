//! The simple timer's source-bound arithmetic before its first queue wrap.
use super::{bootstrap_real, shape, word};
use crate::script_random::real;

#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub origin: f32,
    pub cycle: f32,
    pub minimum: f32,
    pub epsilon: f32,
}

fn call(bytes: &[u8], base: u32, at: usize, target: u32) -> Result<(), String> {
    shape(bytes, at, &[0xe8])?;
    if base
        .wrapping_add(at as u32 + 5)
        .wrapping_add(word(bytes, at + 1)?)
        != target
    {
        return Err("autocast timer numeric call changed".into());
    }
    Ok(())
}

impl Rules {
    pub(super) fn read(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        // The embedded simple timer's virtual +8, rather than the extended JASS timer.
        if word(&read(0xe6e240, 4, false)?, 0)? != 0x7fb700 {
            return Err("autocast simple timer dispatch differs".into());
        }
        shape(
            &read(0x7fb75b, 18, true)?,
            0,
            &[
                0x83, 0x7d, 0x18, 0, 0x74, 0x09, 0x83, 0x4e, 0x10, 0x04, 0x8b, 0x7f, 0x48, 0xeb,
                0x03, 0x8b, 0x7f, 0x44,
            ],
        )?;
        let schedule = read(0x7fb771, 0x85, true)?;
        shape(&schedule, 3, &[0x8b, 0x4f, 0x48])?;
        shape(
            &schedule,
            0x10,
            &[
                0xf3, 0x0f, 0x10, 0x45, 0x18, 0x0f, 0x2f, 0x45, 0xfc, 0x0f, 0x93, 0xc1, 0x85, 0xc9,
                0x74, 0x17,
            ],
        )?;
        shape(&schedule, 0x20, &[0x68])?;
        call(&schedule, 0x7fb771, 0x2d, 0x9a00b0)?;
        shape(&schedule, 0x37, &[0xf3, 0x0f, 0x10, 0x05])?;
        shape(&schedule, 0x4c, &[0x0f, 0x47, 0x05])?;
        if word(&schedule, 0x3b)? != word(&schedule, 0x4f)? {
            return Err("autocast timer minimum operands differ".into());
        }
        shape(&schedule, 0x60, &[0x8d, 0x47, 0x40])?;
        call(&schedule, 0x7fb771, 0x68, 0x9a01c0)?;
        call(&schedule, 0x7fb771, 0x80, 0x7fbe00)?;
        let manager = read(0x7f73cc, 28, true)?;
        shape(&manager, 0, &[0x68])?;
        call(&manager, 0x7f73cc, 7, 0x7f3390)?;
        shape(&manager, 25, &[0x89, 0x47, 0x44])?;
        let ctor = read(0x7f3407, 23, true)?;
        shape(&ctor, 0, &[0xa1])?;
        shape(
            &ctor,
            5,
            &[
                0x89, 0x47, 0x40, 0x8b, 0x45, 0x08, 0xc7, 0x47, 0x44, 0, 0, 0, 0, 0x8b, 0, 0x89,
                0x47, 0x48,
            ],
        )?;
        let scalar = |read: &mut dyn FnMut(u32, usize, bool) -> Result<Vec<u8>, String>, va| {
            let value = f32::from_bits(word(&read(va, 4, false)?, 0)?);
            real::from_f32(value).map_err(|e| format!("autocast timer scalar: {e}"))?;
            Ok::<_, String>(value)
        };
        let rules = Self {
            origin: bootstrap_real(0x401300, word(&ctor, 1)?, read)?,
            cycle: bootstrap_real(0x401570, word(&manager, 1)?, read)?,
            minimum: scalar(read, word(&schedule, 0x3b)?)?,
            epsilon: scalar(read, word(&schedule, 0x21)?)?,
        };
        if rules.origin < 0.
            || rules.cycle <= rules.origin
            || rules.minimum <= 0.
            || rules.epsilon <= 0.
            || rules.epsilon >= rules.cycle
        {
            return Err("autocast timer source domain invalid".into());
        }
        Ok(rules)
    }

    pub fn deadline(&self, now: f64, delay: f32) -> Result<f64, String> {
        let native_now = now as f32;
        if !now.is_finite()
            || f64::from(native_now) != now
            || native_now < self.origin
            || native_now >= self.cycle
        {
            return Err("autocast native clock domain is not admitted".into());
        }
        let arithmetic = || {
            let cycle = real::from_f32(self.cycle)?;
            let requested = real::from_f32(delay)?;
            let delay = if real::below_or_equal(cycle, requested) {
                real::sub(cycle, real::from_f32(self.epsilon)?)?
            } else {
                requested
            };
            let minimum = real::from_f32(self.minimum)?;
            real::add(
                real::from_f32(native_now)?,
                if real::below_or_equal(delay, minimum) {
                    minimum
                } else {
                    delay
                },
            )
        };
        let deadline =
            f32::from_bits(arithmetic().map_err(|e| format!("autocast timer arithmetic: {e}"))?);
        if !deadline.is_finite() || deadline <= native_now || deadline >= self.cycle {
            return Err("autocast queue wrap or non-progressing deadline is not admitted".into());
        }
        Ok(f64::from(deadline))
    }
}

#[cfg(test)]
#[path = "autocast_clock_tests.rs"]
mod tests;

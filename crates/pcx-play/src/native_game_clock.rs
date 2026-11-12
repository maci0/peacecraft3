//! Source game-queue words and nominal pacing; public seconds are a view only.
use crate::{
    autocast_rules::clock,
    neutral_player::{RuleInputBudget, sha256},
    neutral_rule_input::Pe,
    script_numeric::NumericPolicy,
    script_random::real,
};
use pcx_script::RealOps;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub simple: clock::Rules,
    pub numerator: u32,
    pub denominator: u32,
    pub step: u32,
    pub wrap_tolerance: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub cycle: u32,
    pub local: u32,
}

/// One native drain. A cycle-ending drain must rebase requests before completing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub point: Point,
    pub ends_cycle: bool,
}

#[derive(Clone, Copy, Debug)]
struct Pending {
    segment: Segment,
    remainder: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driver {
    Fresh,
    Native,
    Legacy,
}

#[derive(Clone, Debug)]
pub struct State {
    rules: Rules,
    driver: Driver,
    point: Point,
    /// Nanoseconds times the source denominator, including whole unconsumed steps.
    credit: u128,
    pending: Option<Pending>,
    selected: Option<Point>,
}

fn word(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at.saturating_add(4))
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "native game clock operand truncated".into())
}
fn shape(b: &[u8], at: usize, expected: &[u8]) -> Result<(), String> {
    if b.get(at..at.saturating_add(expected.len())) == Some(expected) {
        Ok(())
    } else {
        Err("native game clock instruction relationship changed".into())
    }
}
fn call(b: &[u8], base: u32, at: usize, target: u32) -> Result<(), String> {
    shape(b, at, &[0xe8])?;
    if base
        .wrapping_add(at as u32 + 5)
        .wrapping_add(word(b, at + 1)?)
        == target
    {
        Ok(())
    } else {
        Err("native game clock arithmetic call changed".into())
    }
}

impl Rules {
    pub fn read(
        image: &[u8],
        numeric: &NumericPolicy,
        budget: RuleInputBudget,
    ) -> Result<Self, String> {
        if image.len() > budget.image_bytes
            || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
            || numeric.image_sha256() != &sha256(image)
        {
            return Err("native game clock selected image differs".into());
        }
        let pe = Pe::parse(image, budget).map_err(|e| e.to_string())?;
        let mut read = |va, n, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "native game clock mapped window truncated".into())
        };
        let simple = clock::Rules::read(&mut read)?;
        verify_timed_event(&mut read)?;
        let (numerator, denominator, destination) = decode_step(&read(0x406730, 56, true)?)?;
        let consumer = read(0x5af277, 43, true)?;
        shape(&consumer, 0, &[0xc7, 0x45, 0xd0])?;
        // Native event payload is populated from the same initializer destination.
        shape(&consumer, 21, &[0xa1])?;
        shape(&consumer, 40, &[0x89, 0x45, 0xdc])?;
        if word(&consumer, 3)? != 0x40090092 || word(&consumer, 22)? != destination {
            return Err("native game sub-step event binding changed".into());
        }
        let step = numeric
            .division(
                numeric.integer(numerator as i32)?.to_bits(),
                numeric.integer(denominator as i32)?.to_bits(),
            )
            .map_err(|e| e.to_string())?;
        let wrap_tolerance = decode_wrap(&mut read)?;
        let rules = Self {
            simple,
            numerator,
            denominator,
            step,
            wrap_tolerance,
        };
        rules.validate()?;
        Ok(rules)
    }

    fn validate(&self) -> Result<(), String> {
        for value in [
            self.simple.origin,
            self.simple.cycle,
            self.simple.minimum,
            self.simple.epsilon,
            f32::from_bits(self.step),
            f32::from_bits(self.wrap_tolerance),
        ] {
            real::from_f32(value).map_err(|e| e.to_string())?;
        }
        if self.numerator == 0
            || self.denominator == 0
            || self.numerator > i32::MAX as u32
            || self.denominator > i32::MAX as u32
            || self.simple.origin != 0.
            || self.simple.minimum <= 0.
            || self.simple.epsilon <= 0.
            || self.simple.epsilon >= self.simple.cycle
            || f32::from_bits(self.step) <= 0.
            || f32::from_bits(self.step) >= self.simple.cycle
            || f32::from_bits(self.wrap_tolerance) < 0.
            || f32::from_bits(self.wrap_tolerance) >= f32::from_bits(self.step)
        {
            return Err("native game clock source domain invalid".into());
        }
        Ok(())
    }

    pub fn seconds(&self, point: Point) -> f64 {
        f64::from(point.cycle) * f64::from(self.simple.cycle)
            + f64::from(f32::from_bits(point.local))
    }

    /// Initial simple requests retain this clamped period for later repeats.
    pub fn simple_period(&self, delay: f32) -> Result<u32, String> {
        let requested = real::from_f32(delay).map_err(|e| e.to_string())?;
        let cycle = self.simple.cycle.to_bits();
        let delay = if real::below_or_equal(cycle, requested) {
            real::sub(cycle, self.simple.epsilon.to_bits()).map_err(|e| e.to_string())?
        } else {
            requested
        };
        Ok(
            if real::below_or_equal(delay, self.simple.minimum.to_bits()) {
                self.simple.minimum.to_bits()
            } else {
                delay
            },
        )
    }

    /// Preserve the raw word even when it exceeds the current cycle limit.
    pub fn simple_deadline(&self, now: Point, delay: f32) -> Result<Point, String> {
        self.periodic_deadline(now, self.simple_period(delay)?)
    }

    /// Repeats add the retained period at the selected callback cursor, without reclamping.
    pub fn periodic_deadline(&self, now: Point, period: u32) -> Result<Point, String> {
        let local = real::add(now.local, period).map_err(|e| e.to_string())?;
        if !f32::from_bits(local).is_finite() || real::below_or_equal(local, now.local) {
            return Err("native game clock deadline did not progress".into());
        }
        Ok(Point {
            cycle: now.cycle,
            local,
        })
    }

    /// The caller preflights all requests before publishing any rebased word.
    pub fn rebase(&self, point: Point) -> Result<Point, String> {
        Ok(Point {
            cycle: point
                .cycle
                .checked_add(1)
                .ok_or("native game clock epoch overflow")?,
            local: real::sub(point.local, self.simple.cycle.to_bits())
                .map_err(|e| e.to_string())?,
        })
    }
}

fn decode_step(b: &[u8]) -> Result<(u32, u32, u32), String> {
    shape(
        b,
        0,
        &[0x55, 0x8b, 0xec, 0x83, 0xec, 8, 0x8d, 0x45, 0xfc, 0x68],
    )?;
    shape(b, 14, &[0x50])?;
    call(b, 0x406730, 15, 0x9a1620)?;
    shape(b, 20, &[0x83, 0xc4, 8, 0x50, 0x8d, 0x45, 0xf8, 0x6a])?;
    shape(b, 29, &[0x50])?;
    call(b, 0x406730, 30, 0x9a1620)?;
    shape(b, 35, &[0x83, 0xc4, 8, 0x50, 0x68])?;
    call(b, 0x406730, 44, 0x9a02d0)?;
    shape(b, 49, &[0x83, 0xc4, 12, 0x8b, 0xe5, 0x5d, 0xc3])?;
    let numerator = i32::from(*b.get(28).ok_or("native numerator missing")? as i8);
    let denominator = word(b, 10)? as i32;
    if numerator <= 0 || denominator <= 0 {
        return Err("native game clock ratio must be positive".into());
    }
    Ok((numerator as u32, denominator as u32, word(b, 40)?))
}

fn verify_timed_event(
    read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let native = read(0x4ae060, 0x74, true)?;
    call(&native, 0x4ae060, 0x5e, 0x48cee0)?;
    call(&native, 0x4ae060, 0x6f, 0x5093c0)?;
    call(&read(0x48cee0, 0x19, true)?, 0x48cee0, 0x14, 0x5093a0)?;
    let factory_id = read(0x5093a0, 6, true)?;
    let event_id = read(0x5091e0, 6, true)?;
    shape(&factory_id, 0, &[0xb8])?;
    shape(&event_id, 0, &[0xb8])?;
    if word(&factory_id, 1)? != word(&event_id, 1)?
        || word(&read(0xe7d8b0, 0x20, false)?, 0x1c)? != 0x5091e0
    {
        return Err("native timed-event factory identity differs".into());
    }
    let factory = read(0x405670, 0x2d, true)?;
    if word(&factory, 0x19)? != 0x113c288
        || word(&factory, 0x1d)? != 0xe7d89c
        || word(&read(0xe7d89c, 12, false)?, 0)? != 0x509170
    {
        return Err("native timed-event factory constructor differs".into());
    }
    call(&read(0x509170, 13, true)?, 0x509170, 7, 0x509300)?;
    let ctor = read(0x509300, 0x99, true)?;
    shape(&ctor, 0x4d, &[0x8d, 0x4e, 0x34])?;
    let table = word(&ctor, 0x7e)?;
    if word(&read(table, 12, false)?, 8)? != 0x7fb820 {
        return Err("native timed event is not an extended timer".into());
    }
    let setup = read(0x5093c0, 0x32, true)?;
    shape(&setup, 0x18, &[0x6a, 0])?;
    shape(&setup, 0x1d, &[0x83, 0xc1, 0x34])?;
    call(&setup, 0x5093c0, 0x29, 0x7fbf80)
}

fn decode_wrap(
    read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
) -> Result<u32, String> {
    let b = read(0x7fac96, 0x9a, true)?;
    shape(
        &b,
        0,
        &[
            0xff, 0x75, 8, 0x8d, 0x7e, 0x40, 0x8d, 0x45, 0xfc, 0x57, 0x50,
        ],
    )?;
    call(&b, 0x7fac96, 11, 0x9a01c0)?;
    shape(&b, 16, &[0xf3, 0x0f, 0x10, 0x46, 0x48, 0x8d, 0x5e, 0x48])?;
    shape(&b, 27, &[0x0f, 0x2f, 0x45, 0xfc, 0x76, 0x15])?;
    call(&b, 0x7fac96, 40, 0x7f94a0)?;
    call(&b, 0x7fac96, 63, 0x9a00b0)?;
    shape(&b, 68, &[0x68])?;
    call(&b, 0x7fac96, 81, 0x9a00b0)?;
    shape(&b, 86, &[0xf3, 0x0f, 0x10, 0x05])?;
    shape(&b, 101, &[0x25, 0xff, 0xff, 0xff, 0x7f])?;
    shape(
        &b,
        109,
        &[0x0f, 0x2f, 0x45, 0xf8, 0x8b, 0x45, 0x0c, 0x0f, 0x47, 0x05],
    )?;
    if word(&b, 69)? != word(&b, 119)? {
        return Err("native wrap zero operands differ".into());
    }
    call(&b, 0x7fac96, 130, 0x7f94a0)?;
    call(&b, 0x7fac96, 137, 0x7f9360)?;
    call(&b, 0x7fac96, 149, 0x7f94a0)?;
    let shift = read(0x7f9372, 47, true)?;
    shape(&shift, 0, &[0x8d, 0x4e, 0x48])?;
    shape(&shift, 14, &[0x83, 0xc7, 4, 0x57, 0x50])?;
    call(&shift, 0x7f9372, 19, 0x9a00b0)?;
    shape(
        &shift,
        33,
        &[
            0x89, 0x07, 0x3b, 0x5e, 0x20, 0x72, 0xdc, 0x5f, 0xff, 0x46, 0x44,
        ],
    )?;
    word(&read(word(&b, 90)?, 4, false)?, 0)
}

impl State {
    pub fn new(rules: Rules) -> Result<Self, String> {
        rules.validate()?;
        Ok(Self {
            point: Point {
                cycle: 0,
                local: rules.simple.origin.to_bits(),
            },
            rules,
            driver: Driver::Fresh,
            credit: 0,
            pending: None,
            selected: None,
        })
    }
    pub fn driver(&self) -> Driver {
        self.driver
    }
    pub fn select_legacy(&mut self, advances: bool) -> Result<(), String> {
        if self.driver == Driver::Native {
            return Err("native clock cannot accept a legacy time horizon".into());
        }
        if advances {
            self.driver = Driver::Legacy;
        }
        Ok(())
    }
    pub fn rules(&self) -> &Rules {
        &self.rules
    }
    pub fn point(&self) -> Point {
        self.selected
            .unwrap_or_else(|| self.pending.map_or(self.point, |p| p.segment.point))
    }
    pub fn completed_point(&self) -> Point {
        self.point
    }
    pub fn pace(&mut self, elapsed: Duration) -> Result<(), String> {
        if self.driver == Driver::Legacy {
            return Err("legacy clock cannot enter the native producer".into());
        }
        let credit = self
            .credit
            .checked_add(
                elapsed
                    .as_nanos()
                    .checked_mul(u128::from(self.rules.denominator))
                    .ok_or("native pacing overflow")?,
            )
            .ok_or("native pacing carry overflow")?;
        self.credit = credit;
        self.driver = Driver::Native;
        Ok(())
    }
    pub fn next_segment(&mut self, work: &mut usize) -> Result<Option<Segment>, String> {
        if let Some(pending) = self.pending {
            return Ok(Some(pending.segment));
        }
        let quantum = u128::from(self.rules.numerator) * 1_000_000_000;
        if self.credit < quantum {
            return Ok(None);
        }
        if *work == 0 {
            return Err("native game clock step budget exceeded".into());
        }
        let outer = real::add(self.point.local, self.rules.step).map_err(|e| e.to_string())?;
        if real::below_or_equal(outer, self.point.local) {
            return Err("native game clock step did not progress".into());
        }
        let wrapped = real::below_or_equal(self.rules.simple.cycle.to_bits(), outer);
        let (local, remainder) = if wrapped {
            self.point
                .cycle
                .checked_add(1)
                .ok_or("native game clock epoch overflow")?;
            let remainder =
                real::sub(outer, self.rules.simple.cycle.to_bits()).map_err(|e| e.to_string())?;
            let distance = real::abs(
                real::sub(remainder, self.rules.simple.origin.to_bits())
                    .map_err(|e| e.to_string())?,
            );
            let remainder = if f32::from_bits(distance) < f32::from_bits(self.rules.wrap_tolerance)
            {
                self.rules.simple.origin.to_bits()
            } else {
                remainder
            };
            (self.rules.simple.cycle.to_bits(), Some(remainder))
        } else {
            (outer, None)
        };
        let segment = Segment {
            point: Point {
                cycle: self.point.cycle,
                local,
            },
            ends_cycle: wrapped,
        };
        self.pending = Some(Pending { segment, remainder });
        *work -= 1;
        Ok(Some(segment))
    }
    /// Call after a successful drain, and after all request rebases at a cycle end.
    pub fn finish_segment(&mut self, segment: Segment) -> Result<(), String> {
        let pending = self
            .pending
            .ok_or("native game clock has no pending drain")?;
        if pending.segment != segment || self.selected.is_some() {
            return Err("native game clock drain context mismatch".into());
        }
        if let Some(remainder) = pending.remainder {
            self.pending = Some(Pending {
                segment: Segment {
                    point: Point {
                        cycle: segment
                            .point
                            .cycle
                            .checked_add(1)
                            .ok_or("native game clock epoch overflow")?,
                        local: remainder,
                    },
                    ends_cycle: false,
                },
                remainder: None,
            });
        } else {
            self.point = segment.point;
            self.pending = None;
            self.credit -= u128::from(self.rules.numerator) * 1_000_000_000;
        }
        Ok(())
    }
    pub fn enter_callback(&mut self, point: Point) -> Result<(), String> {
        if self.selected.is_some()
            || point.cycle != self.point().cycle
            || real::from_f32(f32::from_bits(point.local)).is_err()
            || !f32::from_bits(point.local).is_finite()
            || f32::from_bits(point.local) < self.rules.simple.origin
            || !real::below_or_equal(point.local, self.point().local)
        {
            return Err("native game clock callback context mismatch".into());
        }
        self.selected = Some(point);
        Ok(())
    }
    pub fn leave_callback(&mut self) -> Result<(), String> {
        self.selected
            .take()
            .ok_or("native game clock callback is not active")?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "native_game_clock_tests.rs"]
mod tests;

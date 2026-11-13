//! Exact queue words and retained frame progress, with local source reattachment.
use super::*;
use pcx_script::checkpoint::{Reader, Writer};
use sha2::{Digest, Sha256};

impl Rules {
    fn checkpoint_binding(&self) -> [u8; 32] {
        let Self {
            simple,
            numerator,
            denominator,
            step,
            wrap_tolerance,
            frame,
            sleep,
            turn,
        } = self;
        let clock::Rules {
            origin,
            cycle,
            minimum,
            epsilon,
        } = simple;
        let frame::Rules {
            unsigned_correction,
            scale: frame_scale,
            chunk,
        } = frame;
        let sleep::Rules { script, ai } = sleep;
        let turn::Rules {
            quantum_ms,
            credit_cap,
            speed_steps,
            scale,
            modifier,
            elapsed_numerator,
            elapsed_denominator,
            initial_carry,
        } = turn;
        let mut hash = Sha256::new();
        hash.update(b"PC3CLOCK-RULES\0\x01");
        for value in [origin, cycle, minimum, epsilon] {
            hash.update(value.to_bits().to_le_bytes());
        }
        for value in [
            numerator,
            denominator,
            step,
            wrap_tolerance,
            frame_scale,
            chunk,
            quantum_ms,
            credit_cap,
            scale,
            modifier,
            elapsed_numerator,
            elapsed_denominator,
            initial_carry,
        ] {
            hash.update(value.to_le_bytes());
        }
        for value in unsigned_correction {
            hash.update(value.to_le_bytes());
        }
        hash.update([domain_byte(*script), domain_byte(*ai)]);
        hash.update((speed_steps.len() as u64).to_le_bytes());
        for value in speed_steps.iter() {
            hash.update(value.to_le_bytes());
        }
        hash.finalize().into()
    }
}

impl State {
    pub(crate) fn write_checkpoint(&self, out: &mut Writer) -> Result<(), String> {
        self.validate_checkpoint()?;
        let Self {
            rules,
            driver,
            point,
            credit,
            pending,
            selected,
            real,
            frames,
            selected_domain,
            turn_credit_ms,
            turn_carry,
            sub_ms_nanos,
            counter_platform,
            completed_counter_ns,
        } = self;
        out.raw(&rules.checkpoint_binding())?;
        out.byte(match driver {
            Driver::Fresh => 0,
            Driver::Native => 1,
            Driver::Legacy => 2,
        })?;
        point_out(out, *point)?;
        out.raw(&credit.to_le_bytes())?;
        pending_out(out, *pending)?;
        selected_out(out, *selected)?;
        real.write_checkpoint(out)?;
        out.list(frames.len())?;
        out.owned(
            frames
                .len()
                .checked_mul(std::mem::size_of::<Frame>())
                .ok_or("checkpoint frame allocation overflow")?,
        )?;
        for frame in frames.iter() {
            match frame.input {
                FrameSource::Independent(input) => {
                    out.byte(0)?;
                    duration_out(out, input.game_elapsed)?;
                    out.u32(input.real_elapsed_ms)?;
                }
                FrameSource::Counter { raw_ms, duration } => {
                    out.byte(1)?;
                    duration_out(out, duration)?;
                    out.u32(raw_ms)?;
                }
            }
            out.byte(match frame.phase {
                FramePhase::Queued => 0,
                FramePhase::CounterQuanta => 1,
                FramePhase::Game => 2,
                FramePhase::Real => 3,
            })?;
        }
        out.byte(match selected_domain {
            None => 0,
            Some(Domain::Game) => 1,
            Some(Domain::Real) => 2,
        })?;
        out.u32(*turn_credit_ms)?;
        out.u32(*turn_carry)?;
        out.u32(*sub_ms_nanos)?;
        out.boolean(*counter_platform)?;
        out.raw(&completed_counter_ns.to_le_bytes())
    }

    pub(crate) fn read_checkpoint(rules: Rules, input: &mut Reader<'_>) -> Result<Self, String> {
        rules.validate()?;
        if input.raw(32)? != rules.checkpoint_binding() {
            return Err("checkpoint clock policy differs".into());
        }
        let driver = match input.byte()? {
            0 => Driver::Fresh,
            1 => Driver::Native,
            2 => Driver::Legacy,
            _ => return Err("unknown checkpoint clock driver".into()),
        };
        let point = point_in(input)?;
        let credit = u128_in(input)?;
        let pending = pending_in(input)?;
        let selected = selected_in(input)?;
        let real = frame::Queue::read_checkpoint(input)?;
        let count = input.list()?;
        input.owned(
            count
                .checked_mul(std::mem::size_of::<Frame>())
                .ok_or("checkpoint frame allocation overflow")?,
        )?;
        let mut frames = VecDeque::new();
        frames
            .try_reserve_exact(count)
            .map_err(|_| "checkpoint frame allocation failed")?;
        for _ in 0..count {
            let kind = input.byte()?;
            let duration = duration_in(input)?;
            let raw_ms = input.u32()?;
            let source = match kind {
                0 => FrameSource::Independent(FrameInput {
                    game_elapsed: duration,
                    real_elapsed_ms: raw_ms,
                }),
                1 => FrameSource::Counter { raw_ms, duration },
                _ => return Err("unknown checkpoint frame input".into()),
            };
            let phase = match input.byte()? {
                0 => FramePhase::Queued,
                1 => FramePhase::CounterQuanta,
                2 => FramePhase::Game,
                3 => FramePhase::Real,
                _ => return Err("unknown checkpoint frame phase".into()),
            };
            frames.push_back(Frame {
                input: source,
                phase,
            });
        }
        let selected_domain = match input.byte()? {
            0 => None,
            1 => Some(Domain::Game),
            2 => Some(Domain::Real),
            _ => return Err("unknown checkpoint selected domain".into()),
        };
        let next = Self {
            rules,
            driver,
            point,
            credit,
            pending,
            selected,
            real,
            frames: Arc::new(frames),
            selected_domain,
            turn_credit_ms: input.u32()?,
            turn_carry: input.u32()?,
            sub_ms_nanos: input.u32()?,
            counter_platform: input.boolean()?,
            completed_counter_ns: u128_in(input)?,
        };
        next.validate_checkpoint()?;
        Ok(next)
    }

    fn validate_checkpoint(&self) -> Result<(), String> {
        self.rules.validate()?;
        validate_point(&self.rules, self.point)?;
        validate_point(&self.rules, self.real.point)?;
        if self.pending.is_some() {
            if self.credit < u128::from(self.rules.numerator) * 1_000_000_000 {
                return Err("checkpoint pending game step has no credit".into());
            }
            validate_pending(&self.rules, self.point, self.pending, self.rules.step)?;
        }
        let mut total = self.completed_counter_ns;
        let mut credit = self.credit;
        let mut real_frame = None;
        for (index, frame) in self.frames.iter().enumerate() {
            if index != 0 && frame.phase != FramePhase::Queued {
                return Err("checkpoint later frame is already active".into());
            }
            let raw = match frame.input {
                FrameSource::Independent(input) => {
                    if frame.phase == FramePhase::CounterQuanta {
                        return Err("checkpoint independent frame has counter phase".into());
                    }
                    if frame.phase == FramePhase::Queued {
                        credit = credit
                            .checked_add(
                                input
                                    .game_elapsed
                                    .as_nanos()
                                    .checked_mul(u128::from(self.rules.denominator))
                                    .ok_or("checkpoint frame credit overflow")?,
                            )
                            .ok_or("checkpoint frame credit overflow")?;
                    }
                    input.real_elapsed_ms
                }
                FrameSource::Counter { raw_ms, duration } => {
                    if !self.counter_platform {
                        return Err("checkpoint counter frame has no platform admission".into());
                    }
                    let delta = duration
                        .as_nanos()
                        .checked_add(total % 1_000_000)
                        .ok_or("checkpoint counter duration overflow")?;
                    if (delta / 1_000_000) as u32 != raw_ms {
                        return Err("checkpoint counter word differs from retained input".into());
                    }
                    total = total
                        .checked_add(duration.as_nanos())
                        .ok_or("checkpoint counter accounting overflow")?;
                    raw_ms
                }
            };
            let delta = self.rules.frame.delta(raw)?;
            if index == 0 {
                if frame.phase == FramePhase::Real {
                    real_frame = delta;
                }
                if self.pending.is_some() && frame.phase != FramePhase::Game {
                    return Err("checkpoint game segment has no owning phase".into());
                }
            }
        }
        if total % 1_000_000 != u128::from(self.sub_ms_nanos)
            || self.sub_ms_nanos >= 1_000_000
            || self.turn_credit_ms > self.rules.turn.credit_cap
            || (!self.counter_platform
                && (self.completed_counter_ns != 0
                    || self.sub_ms_nanos != 0
                    || self.turn_credit_ms != 0
                    || self.turn_carry != self.rules.turn.initial_carry))
        {
            return Err("checkpoint counter progress is inconsistent".into());
        }
        let carry = f32::from_bits(self.turn_carry);
        if !carry.is_finite()
            || (self.turn_carry != self.rules.turn.initial_carry && !(0. ..1.).contains(&carry))
        {
            return Err("checkpoint turn carry is invalid".into());
        }
        self.real.validate_checkpoint(&self.rules, real_frame)?;
        match (self.selected_domain, self.selected, self.real.selected) {
            (None, None, None) => {}
            (Some(domain), game, real) => {
                let selected = match domain {
                    Domain::Game if real.is_none() => game,
                    Domain::Real if game.is_none() => real,
                    _ => None,
                }
                .ok_or("checkpoint selected queue identity differs")?;
                let mut context = self.clone();
                context.selected = None;
                context.real.selected = None;
                context.selected_domain = None;
                context.enter_domain_callback(domain, selected)?;
            }
            _ => return Err("checkpoint selected queue identity is incomplete".into()),
        }
        if self.driver != Driver::Native {
            let origin = Point {
                cycle: 0,
                local: self.rules.simple.origin.to_bits(),
            };
            if self.point != origin
                || self.real.point != origin
                || self.credit != 0
                || self.pending.is_some()
                || self.selected.is_some()
                || !self.real.checkpoint_idle()
                || !self.frames.is_empty()
                || self.selected_domain.is_some()
                || self.counter_platform
            {
                return Err("checkpoint inactive driver has native progress".into());
            }
        }
        Ok(())
    }
}

pub(super) fn validate_point(rules: &Rules, point: Point) -> Result<(), String> {
    let local = f32::from_bits(point.local);
    real::from_f32(local).map_err(|e| e.to_string())?;
    if local < rules.simple.origin || local > rules.simple.cycle {
        return Err("checkpoint queue point is outside its source cycle".into());
    }
    Ok(())
}
pub(super) fn validate_pending(
    rules: &Rules,
    point: Point,
    pending: Option<Pending>,
    delta: u32,
) -> Result<(), String> {
    let expected = prepare_drain(rules, point, delta)?;
    let Some(actual) = pending else {
        return Ok(());
    };
    let matches = |a: Pending, b: Pending| a.segment == b.segment && a.remainder == b.remainder;
    if matches(actual, expected) {
        return Ok(());
    }
    if let Some(remainder) = expected.remainder {
        let second = Pending {
            segment: Segment {
                point: Point {
                    cycle: expected
                        .segment
                        .point
                        .cycle
                        .checked_add(1)
                        .ok_or("checkpoint cycle overflow")?,
                    local: remainder,
                },
                ends_cycle: false,
            },
            remainder: None,
        };
        if matches(actual, second) {
            return Ok(());
        }
    }
    Err("checkpoint pending segment differs from source split".into())
}
pub(super) fn point_out(out: &mut Writer, point: Point) -> Result<(), String> {
    out.u32(point.cycle)?;
    out.u32(point.local)
}
pub(super) fn point_in(input: &mut Reader<'_>) -> Result<Point, String> {
    Ok(Point {
        cycle: input.u32()?,
        local: input.u32()?,
    })
}
pub(super) fn selected_out(out: &mut Writer, value: Option<Point>) -> Result<(), String> {
    out.boolean(value.is_some())?;
    if let Some(value) = value {
        point_out(out, value)?;
    }
    Ok(())
}
pub(super) fn selected_in(input: &mut Reader<'_>) -> Result<Option<Point>, String> {
    if input.boolean()? {
        point_in(input).map(Some)
    } else {
        Ok(None)
    }
}
pub(super) fn pending_out(out: &mut Writer, pending: Option<Pending>) -> Result<(), String> {
    out.boolean(pending.is_some())?;
    if let Some(pending) = pending {
        point_out(out, pending.segment.point)?;
        out.boolean(pending.segment.ends_cycle)?;
        out.boolean(pending.remainder.is_some())?;
        if let Some(remainder) = pending.remainder {
            out.u32(remainder)?;
        }
    }
    Ok(())
}
pub(super) fn pending_in(input: &mut Reader<'_>) -> Result<Option<Pending>, String> {
    if !input.boolean()? {
        return Ok(None);
    }
    let point = point_in(input)?;
    let ends_cycle = input.boolean()?;
    let remainder = if input.boolean()? {
        Some(input.u32()?)
    } else {
        None
    };
    Ok(Some(Pending {
        segment: Segment { point, ends_cycle },
        remainder,
    }))
}
fn u128_in(input: &mut Reader<'_>) -> Result<u128, String> {
    Ok(u128::from_le_bytes(
        input.raw(16)?.try_into().expect("sixteen bytes"),
    ))
}
fn domain_byte(domain: Domain) -> u8 {
    match domain {
        Domain::Game => 0,
        Domain::Real => 1,
    }
}
fn duration_out(out: &mut Writer, value: Duration) -> Result<(), String> {
    out.u64(value.as_secs())?;
    out.u32(value.subsec_nanos())
}
fn duration_in(input: &mut Reader<'_>) -> Result<Duration, String> {
    let seconds = input.u64()?;
    let nanos = input.u32()?;
    if nanos >= 1_000_000_000 {
        return Err("checkpoint duration nanoseconds are invalid".into());
    }
    Ok(Duration::new(seconds, nanos))
}

#[cfg(test)]
#[path = "native_clock_checkpoint_tests.rs"]
mod tests;

//! Sleep metadata belongs to its original queue domain and current entry.
use super::{Domain, Driver, ObjectHost, Point, SleepRequest};
use pcx_script::checkpoint::{Reader, Writer};
use sha2::{Digest, Sha256};

impl ObjectHost {
    fn sleep_checkpoint_binding(&self) -> Result<[u8; 32], String> {
        let clock = self
            .native_game_clock
            .as_ref()
            .ok_or("checkpoint sleep source clock missing")?;
        let numeric = self
            .numeric
            .as_ref()
            .ok_or("checkpoint sleep numeric source missing")?;
        let rules = clock.rules();
        let mut hash = Sha256::new();
        hash.update(numeric.image_sha256());
        for value in [
            rules.simple.origin,
            rules.simple.cycle,
            rules.simple.minimum,
            rules.simple.epsilon,
        ] {
            hash.update(value.to_bits().to_le_bytes());
        }
        hash.update([domain(rules.sleep.script), domain(rules.sleep.ai)]);
        Ok(hash.finalize().into())
    }

    fn validate_sleep_checkpoint(
        &self,
        request: SleepRequest,
        expected: Domain,
    ) -> Result<(), String> {
        let clock = self
            .native_game_clock
            .as_ref()
            .ok_or("checkpoint sleep source clock missing")?;
        crate::script_random::real::from_f32(f32::from_bits(request.origin.local))
            .map_err(|e| e.to_string())?;
        if request.domain != expected
            || f32::from_bits(request.origin.local) > clock.rules().simple.cycle
            || f32::from_bits(request.origin.local) < clock.rules().simple.origin
        {
            return Err("checkpoint sleep queue domain or origin differs".into());
        }
        crate::script_random::real::from_f32(f32::from_bits(request.delay))
            .map_err(|e| e.to_string())?;
        match (request.epoch, request.point) {
            (None, None)
                if clock.driver() != Driver::Native
                    && request.origin == clock.queue_point(request.domain) =>
            {
                Ok(())
            }
            (Some(epoch), Some(point)) => {
                if clock.driver() != Driver::Native
                    || point.cycle < request.origin.cycle
                    || point.cycle != clock.queue_point(request.domain).cycle
                    || point.local != epoch.entry_deadline().to_bits()
                    || f32::from_bits(point.local) < 0.
                {
                    return Err("checkpoint sleep current entry differs".into());
                }
                let numeric = self
                    .numeric
                    .as_ref()
                    .ok_or("checkpoint sleep numeric source missing")?;
                numeric
                    .timer_queries()
                    .ok_or("checkpoint sleep query source missing")?
                    .validate_checkpoint_sleep(numeric, epoch, request.origin.local, request.delay)
            }
            _ => Err("checkpoint sleep entry is incomplete".into()),
        }
    }

    pub(crate) fn write_sleep_checkpoint(
        &self,
        request: SleepRequest,
        expected: Domain,
        out: &mut Writer,
    ) -> Result<(), String> {
        self.validate_sleep_checkpoint(request, expected)?;
        out.raw(&self.sleep_checkpoint_binding()?)?;
        out.byte(domain(request.domain))?;
        out.u32(request.delay)?;
        point_out(out, request.origin)?;
        out.boolean(request.epoch.is_some())?;
        if let Some(epoch) = request.epoch {
            point_out(
                out,
                request
                    .point
                    .ok_or("checkpoint sleep current point missing")?,
            )?;
            self.numeric
                .as_ref()
                .and_then(|numeric| numeric.timer_queries())
                .ok_or("checkpoint sleep query source missing")?
                .write_checkpoint_epoch(epoch, out)?;
        }
        Ok(())
    }

    pub(crate) fn read_sleep_checkpoint(
        &self,
        input: &mut Reader<'_>,
        expected: Domain,
    ) -> Result<SleepRequest, String> {
        if input.raw(32)? != self.sleep_checkpoint_binding()? {
            return Err("checkpoint sleep source differs".into());
        }
        let domain = match input.byte()? {
            0 => Domain::Game,
            1 => Domain::Real,
            _ => return Err("unknown checkpoint sleep domain".into()),
        };
        let delay = input.u32()?;
        let origin = point_in(input)?;
        let (epoch, point) = if input.boolean()? {
            let point = point_in(input)?;
            let epoch = self
                .numeric
                .as_ref()
                .and_then(|numeric| numeric.timer_queries())
                .ok_or("checkpoint sleep query source missing")?
                .read_checkpoint_epoch(input)?;
            (Some(epoch), Some(point))
        } else {
            (None, None)
        };
        let request = SleepRequest {
            domain,
            delay,
            origin,
            epoch,
            point,
        };
        self.validate_sleep_checkpoint(request, expected)?;
        Ok(request)
    }
}

fn domain(domain: Domain) -> u8 {
    match domain {
        Domain::Game => 0,
        Domain::Real => 1,
    }
}
fn point_out(out: &mut Writer, point: Point) -> Result<(), String> {
    out.u32(point.cycle)?;
    out.u32(point.local)
}
fn point_in(input: &mut Reader<'_>) -> Result<Point, String> {
    Ok(Point {
        cycle: input.u32()?,
        local: input.u32()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        native_game_clock as clock,
        playercolor_host::{EnumHost, Limits},
    };
    use pcx_script::checkpoint::Budget;
    use std::{sync::Arc, time::Duration};

    fn host() -> ObjectHost {
        let numeric = crate::script_numeric::timer_query::tests::policy(8, 0.125)
            .with_native_game_clock_fixture(clock::Rules {
                simple: crate::autocast_rules::clock::Rules {
                    origin: 0.,
                    cycle: 2.,
                    minimum: 0.03125,
                    epsilon: 0.0625,
                },
                numerator: 1,
                denominator: 8,
                step: 0.125f32.to_bits(),
                wrap_tolerance: 0.00001f32.to_bits(),
                frame: clock::frame::Rules {
                    unsigned_correction: [0f64.to_bits(), 4294967296f64.to_bits()],
                    scale: 0.001f32.to_bits(),
                    chunk: 1f32.to_bits(),
                },
                sleep: clock::sleep::Rules {
                    script: Domain::Real,
                    ai: Domain::Game,
                },
                turn: clock::turn::Rules {
                    quantum_ms: 20,
                    credit_cap: 512,
                    speed_steps: Arc::from([1, 3, 5]),
                    scale: 1f32.to_bits(),
                    modifier: 1,
                    elapsed_numerator: 1,
                    elapsed_denominator: 1,
                    initial_carry: 0,
                },
            });
        let common = pcx_script::parse_script("type agent extends handle\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative TimerStart takes timer t, real timeout, boolean periodic, code callback returns nothing\nnative TimerGetElapsed takes timer t returns real\nnative TimerGetRemaining takes timer t returns real\nnative TimerGetTimeout takes timer t returns real\nnative GetExpiredTimer takes nothing returns timer\n").unwrap();
        let script = pcx_script::parse_script("").unwrap();
        let program = Arc::new(
            pcx_script::resolve_with(
                &script,
                &[pcx_script::Prelude::new("synthetic sleep common", &common)],
            )
            .unwrap(),
        );
        let mut host = ObjectHost::checked(
            EnumHost::checked_selected(
                program,
                "synthetic sleep root",
                &[],
                Limits {
                    max_handles: 8,
                    admission_steps: 100,
                    expression_depth: 16,
                    expression_work: 100,
                },
            )
            .unwrap(),
        )
        .unwrap()
        .with_script_numeric(numeric)
        .with_timer_runtime(0.)
        .unwrap();
        host.admit_native_game_clock().unwrap();
        host
    }
    fn budget() -> Budget {
        Budget {
            bytes: 4096,
            items: 128,
            depth: 16,
        }
    }

    #[test]
    fn deferred_sleep_checkpoint_validates_origin_before_native_or_legacy_admission() {
        let mut native = host();
        let request = native
            .prepare_native_sleep(Domain::Game, 20.25, &mut 1)
            .unwrap()
            .unwrap();
        assert_eq!(request.epoch, None);
        let mut out = Writer::new(budget()).unwrap();
        native
            .write_sleep_checkpoint(request, Domain::Game, &mut out)
            .unwrap();
        let bytes = out.finish();
        let state = format!("{:?}", native.test_native_clock_state());
        for (at, value) in [(37, u32::MAX), (41, 0.5f32.to_bits())] {
            let mut corrupt = bytes.clone();
            corrupt[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(
                native
                    .read_sleep_checkpoint(
                        &mut Reader::new(&corrupt, budget()).unwrap(),
                        Domain::Game
                    )
                    .is_err()
            );
            assert_eq!(format!("{:?}", native.test_native_clock_state()), state);
            assert_eq!(native.timer_game_time(), Some(0.));
        }
        let mut input = Reader::new(&bytes, budget()).unwrap();
        let restored = native
            .read_sleep_checkpoint(&mut input, Domain::Game)
            .unwrap();
        input.finish().unwrap();
        assert_eq!(restored, request);
        native
            .pace_native_game_clock(Duration::ZERO, &mut 128)
            .unwrap();
        let admitted = native.activate_native_sleep(restored, &mut 1).unwrap();
        let fresh = native
            .prepare_native_sleep(Domain::Game, 20.25, &mut 1)
            .unwrap()
            .unwrap();
        assert_eq!(admitted, fresh);
        assert_eq!(
            admitted.boundary().unwrap().cycle,
            native.native_queue_point(Domain::Game).unwrap().cycle
        );
        assert!(
            native
                .read_sleep_checkpoint(&mut Reader::new(&bytes, budget()).unwrap(), Domain::Game)
                .is_err()
        );

        let mut legacy = host();
        legacy.select_legacy_clock_domain(20.25, true).unwrap();
        assert!(legacy.next_timer_expiry(20.25, &mut 0).unwrap().is_none());
        let mut input = Reader::new(&bytes, budget()).unwrap();
        let restored = legacy
            .read_sleep_checkpoint(&mut input, Domain::Game)
            .unwrap();
        input.finish().unwrap();
        assert_eq!(restored, request);
        assert_eq!(legacy.timer_game_time(), Some(20.25));
        assert_eq!(legacy.native_game_point(), None);
    }

    #[test]
    fn sleep_checkpoint_preserves_native_entry_rebase_and_refuses_foreign_domain_or_corrupt_request()
     {
        let mut host = host();
        assert!(
            host.pace_native_game_clock(Duration::ZERO, &mut 128)
                .unwrap()
        );
        let mut request = host
            .prepare_native_sleep(Domain::Game, 20.25, &mut 1)
            .unwrap()
            .unwrap();
        host.pace_native_game_clock(Duration::from_secs(2), &mut 128)
            .unwrap();
        let mut work = 128;
        while let Some((segment, until)) = host.next_native_game_segment(&mut work).unwrap() {
            assert!(host.next_timer_expiry(until, &mut 0).unwrap().is_none());
            if segment.ends_cycle {
                host.rebase_native_sleep(&mut request).unwrap();
            }
            host.finish_native_game_segment(segment, &mut work).unwrap();
        }
        assert_eq!(request.origin, Point { cycle: 0, local: 0 });
        assert_eq!(
            request.boundary(),
            Some(Point {
                cycle: 1,
                local: 6f32.to_bits()
            })
        );
        let mut out = Writer::new(budget()).unwrap();
        host.write_sleep_checkpoint(request, Domain::Game, &mut out)
            .unwrap();
        let bytes = out.finish();
        let mut input = Reader::new(&bytes, budget()).unwrap();
        let mut restored = host
            .read_sleep_checkpoint(&mut input, Domain::Game)
            .unwrap();
        input.finish().unwrap();
        assert_eq!(restored, request);
        assert_eq!(
            host.advance_native_sleep(&mut request).unwrap(),
            host.advance_native_sleep(&mut restored).unwrap()
        );
        assert_eq!(restored, request);
        for cut in 0..bytes.len() {
            assert!(
                host.read_sleep_checkpoint(
                    &mut Reader::new(&bytes[..cut], budget()).unwrap(),
                    Domain::Game
                )
                .is_err()
            );
        }
        assert!(
            host.read_sleep_checkpoint(&mut Reader::new(&bytes, budget()).unwrap(), Domain::Real)
                .is_err()
        );
        let mut corrupt = bytes.clone();
        corrupt[33..37].copy_from_slice(&21.25f32.to_bits().to_le_bytes());
        assert!(
            host.read_sleep_checkpoint(&mut Reader::new(&corrupt, budget()).unwrap(), Domain::Game)
                .is_err()
        );
        let mut corrupt = bytes.clone();
        corrupt[45] = 0;
        assert!(
            host.read_sleep_checkpoint(&mut Reader::new(&corrupt, budget()).unwrap(), Domain::Game)
                .is_err()
        );
        let mut corrupt = bytes.clone();
        corrupt[46..50].copy_from_slice(&2u32.to_le_bytes());
        assert!(
            host.read_sleep_checkpoint(&mut Reader::new(&corrupt, budget()).unwrap(), Domain::Game)
                .is_err()
        );
    }
}

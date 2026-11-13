//! AI continuations retain legacy deadlines separately from source Game entries.
use super::{AiHost, AiWorld, MAX_SLEEPERS, Runtime};
use crate::native_game_clock::{Domain, Point, QueueError, SleepRequest};
use pcx_script::Value;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AiSleep {
    pub(super) legacy_deadline: Option<f64>,
    pub(super) native: Option<SleepRequest>,
}

pub(crate) struct PreparedAiSleepRows {
    rows: Vec<(i32, u64, AiSleep)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AiWake {
    pub player: i32,
    pub ticket: u64,
    pub point: Point,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AiWakeDelivery {
    Internal,
    RetriedPaused,
    Resumed,
}

fn combined<T>(result: Result<T, String>, cleanup: Result<(), String>) -> Result<T, String> {
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error}; callback cleanup: {cleanup}")),
    }
}

impl<W: AiWorld> AiHost<W> {
    pub(super) fn suspend_sleep(&mut self, args: &[Value]) -> bool {
        let [seconds] = args else { return false };
        let Some(seconds) = seconds.as_real() else {
            return false;
        };
        let result = (|| {
            if self.sleepers.len() >= MAX_SLEEPERS {
                return Err("Sleep: thread budget".into());
            }
            let world = self.world()?;
            // The shared helper prepares only the current entry in constant work.
            let mut work = 1;
            let native = world.ai_prepare_native_sleep(Domain::Game, seconds, &mut work)?;
            if native.is_some_and(|request| request.domain() != Domain::Game) {
                return Err("Sleep: AI source queue family differs".into());
            }
            let legacy_deadline = world.ai_clock().and_then(|now| {
                let deadline = now + f64::from(seconds);
                (seconds > 0.0 && deadline.is_finite()).then_some(deadline)
            });
            // Fresh still follows the old legacy call contract until native selection.
            if legacy_deadline.is_none()
                && !native.is_some_and(|request| request.boundary().is_some())
            {
                return Err("Sleep: invalid duration or canonical clock".into());
            }
            Ok(AiSleep {
                legacy_deadline,
                native,
            })
        })();
        let accepted = result.is_ok();
        self.pending_sleep = Some(result);
        accepted
    }

    pub(super) fn sleep_refusal(&mut self) -> String {
        match self.pending_sleep.take() {
            Some(Err(error)) => error,
            other => {
                self.pending_sleep = other;
                "Sleep: invalid duration or thread budget".into()
            }
        }
    }

    pub(super) fn park_sleep(&mut self) -> Result<u64, String> {
        let sleep = *self
            .pending_sleep
            .as_ref()
            .ok_or("Sleep: no pending deadline")?
            .as_ref()
            .map_err(Clone::clone)?;
        let ticket = self
            .next_ticket
            .checked_add(1)
            .ok_or("Sleep: ticket budget")?;
        self.sleepers.insert(ticket, sleep);
        self.next_ticket = ticket;
        self.pending_sleep = None;
        Ok(ticket)
    }

    pub(super) fn resume_sleep(&mut self, ticket: u64) -> Result<(), String> {
        let sleep = self.sleepers.get(&ticket).ok_or("Sleep: unknown ticket")?;
        if let Some(mut request) = sleep.native
            && let Some(point) = request.boundary()
        {
            if request.domain() != Domain::Game
                || self.world()?.ai_native_queue_point(Domain::Game) != Some(point)
            {
                return Err("Sleep: native Game callback point differs".into());
            }
            if !self.world()?.ai_advance_native_sleep(&mut request)? {
                return Err("Sleep: native internal entry cannot resume the VM".into());
            }
        } else {
            let deadline = sleep
                .legacy_deadline
                .ok_or("Sleep: legacy deadline is absent")?;
            if self.world()?.ai_clock().is_none_or(|now| now < deadline) {
                return Err("Sleep: game clock has not reached the wake deadline".into());
            }
        }
        self.sleepers.remove(&ticket);
        Ok(())
    }
}

impl<W: AiWorld> Runtime<W> {
    pub(crate) fn prepare_native_admission(
        &mut self,
        world: &W,
        work: &mut usize,
    ) -> Result<PreparedAiSleepRows, String> {
        let mut rows = Vec::new();
        for (&player, vm) in &mut self.active {
            if self.pending_retirement.contains(&player) {
                continue;
            }
            for (&ticket, sleep) in &vm.host().sleepers {
                *work = work.checked_sub(1).ok_or("AI sleep admission work")?;
                let request = sleep
                    .native
                    .ok_or("AI sleep has no native source request")?;
                if request.domain() != Domain::Game {
                    return Err("AI sleep source queue family differs".into());
                }
                let mut next = *sleep;
                next.native = Some(world.ai_activate_native_sleep(request, work)?);
                if next
                    .native
                    .is_none_or(|request| request.boundary().is_none())
                {
                    return Err("AI sleep native activation has no current entry".into());
                }
                rows.try_reserve(1)
                    .map_err(|_| "AI sleep admission capacity")?;
                rows.push((player, ticket, next));
            }
        }
        Ok(PreparedAiSleepRows { rows })
    }

    pub(crate) fn prepare_native_rebase(
        &mut self,
        world: &W,
        domain: Domain,
        work: &mut usize,
    ) -> Result<PreparedAiSleepRows, String> {
        let mut rows = Vec::new();
        for (&player, vm) in &mut self.active {
            if self.pending_retirement.contains(&player) {
                continue;
            }
            for (&ticket, sleep) in &vm.host().sleepers {
                *work = work.checked_sub(1).ok_or("AI sleep rebase work")?;
                let mut request = sleep
                    .native
                    .ok_or("AI sleep has no native source request")?;
                if request.domain() != domain {
                    continue;
                }
                world.ai_rebase_native_sleep(&mut request)?;
                let mut next = *sleep;
                next.native = Some(request);
                rows.try_reserve(1)
                    .map_err(|_| "AI sleep rebase capacity")?;
                rows.push((player, ticket, next));
            }
        }
        Ok(PreparedAiSleepRows { rows })
    }

    /// The outer owner commits only after clock and all AI preflights succeed.
    pub(crate) fn commit_native_sleep_rows(&mut self, prepared: PreparedAiSleepRows) {
        for (player, ticket, sleep) in prepared.rows {
            *self
                .active
                .get_mut(&player)
                .expect("preflight retains AI")
                .host()
                .sleepers
                .get_mut(&ticket)
                .expect("preflight retains AI ticket") = sleep;
        }
    }

    pub(crate) fn next_native_wake(
        &mut self,
        until: Point,
        work: &mut usize,
    ) -> Result<Option<AiWake>, QueueError> {
        let mut chosen: Option<AiWake> = None;
        for (&player, vm) in &mut self.active {
            *work = work
                .checked_sub(1)
                .ok_or(QueueError::Budget("AI native selection"))?;
            if self.pending_retirement.contains(&player) {
                continue;
            }
            for (&ticket, sleep) in &vm.host().sleepers {
                *work = work
                    .checked_sub(1)
                    .ok_or(QueueError::Budget("AI native selection"))?;
                let request = sleep
                    .native
                    .ok_or("AI sleep has no native source request")?;
                if request.domain() != Domain::Game {
                    return Err("AI sleep source queue family differs".into());
                }
                let point = request
                    .boundary()
                    .ok_or("AI sleep is not natively activated")?;
                if point.cycle != until.cycle {
                    return Err("AI sleep and Game queue cycles differ".into());
                }
                let local = f32::from_bits(point.local);
                if local > f32::from_bits(until.local) {
                    continue;
                }
                let candidate = AiWake {
                    player,
                    ticket,
                    point,
                };
                // Retain the house player/ticket tie policy; native equal-entry order is separate evidence.
                if chosen.is_none_or(|previous| {
                    local
                        .total_cmp(&f32::from_bits(previous.point.local))
                        .then(player.cmp(&previous.player))
                        .then(ticket.cmp(&previous.ticket))
                        .is_lt()
                }) {
                    chosen = Some(candidate);
                }
            }
        }
        Ok(chosen)
    }

    pub(crate) fn deliver_native_wake(
        &mut self,
        world: &mut Option<W>,
        wake: AiWake,
        work: &mut usize,
    ) -> Result<AiWakeDelivery, String> {
        if self.pending_retirement.contains(&wake.player) {
            return Err("campaign AI context is pending retirement".into());
        }
        let vm = self
            .active
            .get_mut(&wake.player)
            .ok_or("campaign AI player is not running")?;
        let sleep = *vm
            .host()
            .sleepers
            .get(&wake.ticket)
            .ok_or("Sleep: unknown ticket")?;
        let mut request = sleep
            .native
            .ok_or("AI sleep has no native source request")?;
        if request.domain() != Domain::Game || request.boundary() != Some(wake.point) {
            return Err("AI selected wake entry changed".into());
        }
        *work = work.checked_sub(1).ok_or("AI native wake work")?;
        let borrowed = world.as_mut().ok_or("campaign AI world is already lent")?;
        if !borrowed.ai_advance_native_sleep(&mut request)? {
            let next = vm
                .host()
                .sleepers
                .get_mut(&wake.ticket)
                .expect("checked sleep");
            next.native = Some(request);
            return Ok(AiWakeDelivery::Internal);
        }
        borrowed.ai_begin_native_domain_callback(Domain::Game, wake.point)?;
        if vm.host().pause_depth.is_some_and(|depth| depth != 0) {
            let result = (|| {
                let retry = self.pause_rules.as_ref().map_err(Clone::clone)?.retry_word;
                let request = borrowed
                    .ai_prepare_native_sleep(Domain::Game, f32::from_bits(retry), work)?
                    .ok_or("PauseCompAI native source request is absent")?;
                if request.domain() != Domain::Game || request.boundary().is_none() {
                    return Err("PauseCompAI retry has no native Game entry".into());
                }
                Ok(request)
            })();
            let request = combined(result, borrowed.ai_end_native_domain_callback())?;
            vm.host()
                .sleepers
                .get_mut(&wake.ticket)
                .expect("paused ticket remains")
                .native = Some(request);
            return Ok(AiWakeDelivery::RetriedPaused);
        }
        let borrowed = world.take().expect("checked canonical world");
        let mut vm = self
            .active
            .remove(&wake.player)
            .expect("checked AI context");
        vm.host().world = Some(borrowed);
        let result = vm
            .resume_thread(wake.ticket)
            .map_err(|e| format!("campaign AI player {}: {e}", wake.player));
        *world = vm.host().world.take();
        let returned = world.as_mut().expect("AI returns its canonical world");
        let cleanup = returned.ai_end_native_domain_callback();
        if result.is_ok() {
            self.active.insert(wake.player, vm);
        } else if let Err(error) = returned.ai_deactivate_player(wake.player) {
            self.active.insert(wake.player, vm);
            self.pending_retirement.insert(wake.player);
            return combined(
                Err(format!(
                    "{}; retirement: {error}",
                    result.as_ref().expect_err("failed AI execution")
                )),
                cleanup,
            );
        }
        combined(result.map(|()| AiWakeDelivery::Resumed), cleanup)
    }
}

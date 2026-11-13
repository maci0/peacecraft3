//! Stable AI section; source readers and the canonical world are rebound by the candidate owner.
use super::native_sleep::AiSleep;
use super::{AiHost, AiSource, AiWorld, MAX_PLAYERS, MAX_SLEEPERS, Runtime};
use pcx_assets::AssetPath;
use pcx_script::checkpoint::{Budget, Reader, Writer};
use pcx_script::{
    OwnedVm,
    eval::{CheckpointProjection, CheckpointReference},
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

fn result_out(out: &mut Writer, result: &Result<i32, String>) -> Result<(), String> {
    out.boolean(result.is_ok())?;
    match result {
        Ok(value) => out.i32(*value),
        Err(error) => out.text(error),
    }
}
fn result_in(input: &mut Reader<'_>) -> Result<Result<i32, String>, String> {
    Ok(if input.boolean()? {
        Ok(input.i32()?)
    } else {
        Err(input.text()?)
    })
}
fn pause_binding(rules: &crate::campaign_ai_pause_policy::Rules) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(rules.initial.to_le_bytes());
    hash.update(rules.retry_seconds.to_bits().to_le_bytes());
    hash.update(rules.retry_word.to_le_bytes());
    hash.finalize().into()
}
fn sleep_out<W: AiWorld>(world: &W, out: &mut Writer, sleep: AiSleep) -> Result<(), String> {
    if sleep.legacy_deadline.is_none() && sleep.native.is_none() {
        return Err("checkpoint AI sleeper has no deadline".into());
    }
    out.boolean(sleep.legacy_deadline.is_some())?;
    if let Some(deadline) = sleep.legacy_deadline {
        if !deadline.is_finite() {
            return Err("checkpoint AI deadline is not finite".into());
        }
        out.u64(deadline.to_bits())?;
    }
    out.boolean(sleep.native.is_some())?;
    if let Some(request) = sleep.native {
        world.ai_write_sleep_checkpoint(request, out)?;
    }
    Ok(())
}
fn sleep_in<W: AiWorld>(world: &W, input: &mut Reader<'_>) -> Result<AiSleep, String> {
    let legacy_deadline = if input.boolean()? {
        let deadline = f64::from_bits(input.u64()?);
        if !deadline.is_finite() {
            return Err("checkpoint AI deadline is not finite".into());
        }
        Some(deadline)
    } else {
        None
    };
    let native = if input.boolean()? {
        Some(world.ai_read_sleep_checkpoint(input)?)
    } else {
        None
    };
    if legacy_deadline.is_none() && native.is_none() {
        return Err("checkpoint AI sleeper has no deadline".into());
    }
    Ok(AiSleep {
        legacy_deadline,
        native,
    })
}

impl<W: AiWorld> Runtime<W> {
    pub(crate) fn write_checkpoint(
        &mut self,
        world: &W,
        out: &mut Writer,
        vm_budget: Budget,
    ) -> Result<(), String> {
        let Self {
            loader,
            active,
            pending_retirement,
            remaining_loads,
            pause_rules,
            melee_difficulty,
        } = self;
        if active.len() > MAX_PLAYERS
            || pending_retirement
                .iter()
                .any(|player| !active.contains_key(player))
        {
            return Err("checkpoint AI context ownership is incomplete".into());
        }
        out.boolean(loader.is_some())?;
        out.u64(u64::try_from(*remaining_loads).map_err(|_| "checkpoint AI load budget")?)?;
        out.boolean(pause_rules.is_ok())?;
        match pause_rules {
            Ok(rules) => out.raw(&pause_binding(rules))?,
            Err(error) => out.text(error)?,
        }
        out.boolean(melee_difficulty.is_some())?;
        if let Some(input) = melee_difficulty {
            input.write_checkpoint(out)?;
        }
        out.list(pending_retirement.len())?;
        for player in pending_retirement.iter() {
            out.i32(*player)?;
        }
        out.list(active.len())?;
        for (&player, vm) in active {
            let AiHost {
                player: host_player,
                source,
                melee_difficulty,
                world: lent,
                pending_sleep,
                pause_depth,
                sleepers,
                next_ticket,
            } = vm.host();
            if *host_player != player || lent.is_some() || sleepers.len() > MAX_SLEEPERS {
                return Err("checkpoint AI world is lent or host identity differs".into());
            }
            let source = source
                .as_ref()
                .ok_or("checkpoint AI context has no pinned source")?;
            out.i32(player)?;
            out.text(source.requested.as_str())?;
            out.text(&source.scripts.identity.edition)?;
            out.text(&source.scripts.identity.version)?;
            let fingerprint = source.scripts.source_fingerprint();
            out.raw(&fingerprint)?;
            result_out(out, melee_difficulty)?;
            out.boolean(pause_depth.is_some())?;
            if let Some(depth) = pause_depth {
                out.i32(*depth)?;
            }
            out.u64(*next_ticket)?;
            out.boolean(pending_sleep.is_some())?;
            if let Some(sleep) = pending_sleep {
                out.boolean(sleep.is_ok())?;
                match sleep {
                    Ok(sleep) => sleep_out(world, out, *sleep)?,
                    Err(error) => out.text(error)?,
                }
            }
            out.list(sleepers.len())?;
            for (&ticket, sleep) in sleepers {
                if ticket == 0 || ticket > *next_ticket {
                    return Err("checkpoint AI sleeper ticket differs".into());
                }
                out.u64(ticket)?;
                sleep_out(world, out, *sleep)?;
            }
            let bytes =
                vm.checkpoint_state_bytes(fingerprint, CheckpointProjection::Idle, vm_budget)?;
            out.blob(&bytes)?;
        }
        Ok(())
    }

    /// Only an unpublished, fresh runtime may receive this section. No AI entrypoint runs.
    pub(crate) fn restore_checkpoint<F, P, C>(
        &mut self,
        world: &W,
        input: &mut Reader<'_>,
        vm_budget: Budget,
        mut player_valid: P,
        mut context_valid: C,
        mut reference: F,
    ) -> Result<(), String>
    where
        F: for<'a> FnMut(CheckpointReference<'a>) -> Result<(), String>,
        P: FnMut(i32) -> Result<(), String>,
        C: FnMut(i32) -> Result<(), String>,
    {
        if !self.active.is_empty() || !self.pending_retirement.is_empty() {
            return Err("checkpoint AI restoration requires a fresh candidate runtime".into());
        }
        let has_loader = input.boolean()?;
        if has_loader && self.loader.is_none() {
            return Err("checkpoint AI source reader is missing".into());
        }
        let remaining_loads =
            usize::try_from(input.u64()?).map_err(|_| "checkpoint AI load budget")?;
        if remaining_loads > self.remaining_loads {
            return Err("checkpoint AI load budget exceeds candidate policy".into());
        }
        let pause_rules = if input.boolean()? {
            let rules = self.pause_rules.as_ref().map_err(Clone::clone)?;
            if input.raw(32)? != pause_binding(rules) {
                return Err("checkpoint AI pause source differs".into());
            }
            Ok(rules.clone())
        } else {
            if self.pause_rules.is_ok() {
                return Err("checkpoint AI pause admission differs".into());
            }
            Err(input.text()?)
        };
        let melee_difficulty = if input.boolean()? {
            Some(
                self.melee_difficulty
                    .as_ref()
                    .ok_or("checkpoint future AI difficulty source is missing")?
                    .read_checkpoint(input, &mut player_valid)?,
            )
        } else {
            None
        };
        let count = input.list()?;
        if count > MAX_PLAYERS {
            return Err("checkpoint AI retirement budget".into());
        }
        input.owned(
            count
                .checked_mul(std::mem::size_of::<i32>())
                .ok_or("checkpoint AI retirement allocation")?,
        )?;
        let mut pending_retirement = BTreeSet::new();
        for _ in 0..count {
            let player = input.i32()?;
            context_valid(player)?;
            if !pending_retirement.insert(player) {
                return Err("duplicate checkpoint AI retirement".into());
            }
        }
        let count = input.list()?;
        if count > MAX_PLAYERS {
            return Err("checkpoint AI context budget".into());
        }
        input.owned(
            count
                .checked_mul(std::mem::size_of::<(i32, OwnedVm<AiHost<W>>)>())
                .ok_or("checkpoint AI context allocation")?,
        )?;
        let mut active = BTreeMap::new();
        for _ in 0..count {
            let player = input.i32()?;
            context_valid(player)?;
            if active.contains_key(&player) {
                return Err("duplicate checkpoint AI player".into());
            }
            let path = input.text()?;
            if path.len() > 16 << 10 {
                return Err("checkpoint AI path exceeds source limit".into());
            }
            let requested = AssetPath::new(&path).map_err(|e| e.to_string())?;
            if requested.as_str() != path {
                return Err("checkpoint AI path is not canonical".into());
            }
            let edition = input.text()?;
            let version = input.text()?;
            let fingerprint: [u8; 32] = input
                .raw(32)?
                .try_into()
                .map_err(|_| "checkpoint AI source fingerprint")?;
            let scripts = self
                .loader
                .as_mut()
                .ok_or("checkpoint AI source reader is missing")?(
                requested.as_str()
            )?;
            super::admit_ai_natives(&scripts)?;
            if scripts.source_fingerprint() != fingerprint
                || scripts.identity.edition != edition
                || scripts.identity.version != version
            {
                return Err("checkpoint AI source differs from its pinned context".into());
            }
            let captured = result_in(input)?;
            let pause_depth = if input.boolean()? {
                Some(input.i32()?)
            } else {
                None
            };
            let next_ticket = input.u64()?;
            let pending_sleep = if input.boolean()? {
                Some(if input.boolean()? {
                    Ok(sleep_in(world, input)?)
                } else {
                    Err(input.text()?)
                })
            } else {
                None
            };
            let count = input.list()?;
            if count > MAX_SLEEPERS {
                return Err("checkpoint AI sleeper budget".into());
            }
            input.owned(
                count
                    .checked_mul(std::mem::size_of::<(u64, AiSleep)>())
                    .ok_or("checkpoint AI sleeper allocation")?,
            )?;
            let mut sleepers = BTreeMap::new();
            for _ in 0..count {
                let ticket = input.u64()?;
                if ticket == 0 || ticket > next_ticket || sleepers.contains_key(&ticket) {
                    return Err("checkpoint AI sleeper ticket differs".into());
                }
                sleepers.insert(ticket, sleep_in(world, input)?);
            }
            let bytes = input.blob()?;
            let host = AiHost {
                player,
                source: Some(AiSource {
                    requested,
                    scripts: Arc::clone(&scripts),
                }),
                melee_difficulty: captured,
                world: None,
                pending_sleep,
                pause_depth,
                sleepers,
                next_ticket,
            };
            let mut vm = OwnedVm::new_owned(scripts.shared_program(), host);
            let mut seen = BTreeSet::new();
            let projection =
                vm.restore_checkpoint_state(&bytes, fingerprint, vm_budget, |host, item| {
                    if let CheckpointReference::Thread(ticket) = item {
                        if !host.sleepers.contains_key(&ticket) {
                            return Err("checkpoint AI VM has no matching host sleeper".into());
                        }
                        seen.insert(ticket);
                        Ok(())
                    } else {
                        reference(item)
                    }
                })?;
            if projection != CheckpointProjection::Idle {
                return Err("checkpoint active AI save projection is not qualified".into());
            }
            if seen.len() != vm.host().sleepers.len() {
                return Err("checkpoint AI host sleeper has no matching VM thread".into());
            }
            active.insert(player, vm);
        }
        if pending_retirement
            .iter()
            .any(|player| !active.contains_key(player))
        {
            return Err("checkpoint AI retirement has no owned context".into());
        }
        self.active = active;
        self.pending_retirement = pending_retirement;
        self.remaining_loads = remaining_loads;
        self.pause_rules = pause_rules;
        self.melee_difficulty = melee_difficulty;
        if !has_loader {
            self.loader = None;
        }
        Ok(())
    }
}

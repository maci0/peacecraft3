//! Source-timed Slow phases and callback settlement, shared by manual and automatic orders.
use super::*;
use pcx_script::{Host, ScriptCall};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Accepted,
    Effect,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    caster: HandleId,
    revision: u64,
    phase: Phase,
}

pub(super) struct Cast {
    revision: u64,
    backswing: f64,
}

#[derive(Clone)]
struct Payment {
    caster: HandleId,
    target: HandleId,
    alias: [u8; 4],
    rank: i32,
    cost: f32,
    minimum: f32,
    withdrawn: bool,
    cooldown: f64,
    buff: [u8; 4],
    movement: f32,
    attack: f32,
    expires_at: f64,
}

pub(super) struct Acceptance {
    revision: u64,
    backswing: f64,
    channel: Vec<super::super::combat::AbilityEvent>,
    cast: Vec<super::super::combat::AbilityEvent>,
}

#[derive(Default)]
pub(super) struct State {
    revision: u64,
    payments: BTreeMap<u64, Payment>,
    pub(super) fault: Option<String>,
}

impl ObjectHost {
    /// Proves the zero-DataC no-target case before the still-unqualified positive selector.
    pub fn advance_slow_autocast(
        &mut self,
        caster: HandleId,
        alias: [u8; 4],
        now: f64,
        work: &mut usize,
    ) -> Result<Option<crate::autocast_rules::Outcome>, Error> {
        if self.curse_class(alias)? != SLOW_CLASS {
            return Ok(None);
        }
        if self.timer_game_time() != Some(now) || !now.is_finite() {
            return Err(self.curse_error("Slow evaluation clock differs"));
        }
        if self.curses.is_none() {
            return Err(self.curse_error("Slow effects are not admitted"));
        }
        let unit = self.executed_unit(caster)?;
        if unit.paused_since.is_some() || unit.combat.as_ref().is_none_or(|c| c.dead) {
            return Err(self.curse_error("Slow automatic caster lifecycle is unqualified"));
        }
        let rules = self
            .admitted_autocast_rules()
            .map_err(|e| self.curse_error(e))?
            .slow
            .as_ref()
            .ok_or_else(|| self.curse_error("Slow acquisition source is not admitted"))?;
        if rules.class != SLOW_CLASS {
            return Err(self.curse_error("Slow acquisition class differs"));
        }
        let rank = self
            .executed_unit(caster)?
            .abilities
            .as_ref()
            .ok_or_else(|| self.curse_error("Slow ability state is not admitted"))?
            .level(alias);
        let cell = self
            .hero_ability_level_cell_including_marker(
                alias,
                &rules.data_column,
                rank,
                FieldKind::Bool,
            )
            .map_err(|e| self.curse_error(e))?;
        if !rules
            .requires_attack_target(&cell)
            .map_err(|e| self.curse_error(e))?
        {
            return Err(
                self.curse_error("Slow acquisition without the attack-target gate is unqualified")
            );
        }
        for (_, unit) in self.executed_units() {
            *work = work
                .checked_sub(1)
                .ok_or_else(|| self.curse_error("Slow acquisition work budget exhausted"))?;
            let combat = unit
                .combat
                .as_ref()
                .ok_or_else(|| self.curse_error("Slow candidate attack state is not admitted"))?;
            if combat.target.is_some() {
                return Err(self.curse_error("Slow has a potential attacking candidate; positive selection and turning are unqualified"));
            }
        }
        Ok(Some(crate::autocast_rules::Outcome::Retry))
    }

    fn slow_mana_minimum(&self) -> Result<f32, Error> {
        self.admitted_autocast_rules()
            .map_err(|e| self.curse_error(e))?
            .mana_minimum
            .filter(|v| v.is_finite())
            .ok_or_else(|| self.curse_error("Slow mana minimum is not admitted"))
    }
    fn slow_rows(
        &mut self,
        caster: HandleId,
        phase: &str,
    ) -> Result<Vec<super::super::combat::AbilityEvent>, Error> {
        self.prepare_ability_events(
            caster,
            &format!("EVENT_UNIT_SPELL_{phase}"),
            &format!("EVENT_PLAYER_UNIT_SPELL_{phase}"),
        )
    }

    fn slow_subject(
        caster: HandleId,
        alias: [u8; 4],
        revision: u64,
        phase: Phase,
        target: Option<HandleId>,
    ) -> EventSubject {
        EventSubject {
            unit: Some(caster),
            spell_ability: Some(i32::from_be_bytes(alias)),
            spell_target: target,
            slow: Some(Context {
                caster,
                revision,
                phase,
            }),
            ..Default::default()
        }
    }

    pub(super) fn prepare_slow_windup(&self, caster: HandleId) -> Result<f64, Error> {
        self.slow_mana_minimum()?;
        let d = &self.executed_unit(caster)?.definition;
        let cell = combat::column(d, SlkId::UnitWeapons, "castbsw")
            .map_err(|e| self.curse_error(e))?
            .ok_or_else(|| self.curse_error("Slow cast backswing is missing"))?;
        let seconds = combat::number(cell, "castbsw")
            .map_err(|e| self.curse_error(e))?
            .ok_or_else(|| self.curse_error("Slow cast backswing is empty"))?;
        if !seconds.is_finite() || seconds < 0. {
            return Err(self.curse_error("Slow cast backswing is invalid"));
        }
        Ok(f64::from(seconds))
    }

    pub(super) fn prepare_slow_acceptance(
        &mut self,
        caster: HandleId,
    ) -> Result<Acceptance, Error> {
        let backswing = self.prepare_slow_windup(caster)?;
        let channel = self.slow_rows(caster, "CHANNEL")?;
        let cast = self.slow_rows(caster, "CAST")?;
        let interrupt = self
            .curses
            .as_ref()
            .and_then(|s| s.pending.get(&caster))
            .is_some_and(|c| c.slow.is_some() && !matches!(c.stage, Stage::SlowEnding));
        let interrupt = if interrupt {
            self.slow_rows(caster, "ENDCAST")?.len()
        } else {
            0
        };
        self.reserve_game_event_delivery(channel.len() + cast.len() + interrupt)?;
        let revision = self
            .curses
            .as_ref()
            .expect("curse admission")
            .slow_lifecycle
            .revision
            .checked_add(1)
            .ok_or_else(|| self.inner.error(NATIVE, Failure::Capacity))?;
        Ok(Acceptance {
            revision,
            backswing,
            channel,
            cast,
        })
    }

    pub(super) fn begin_slow_windup(&mut self, caster: HandleId, acceptance: Acceptance) {
        let Acceptance {
            revision,
            backswing,
            channel,
            cast,
        } = acceptance;
        let state = self.curses.as_mut().expect("curse admission");
        state.slow_lifecycle.revision = revision;
        let pending = state.pending.get_mut(&caster).expect("Slow windup");
        pending.slow = Some(Cast {
            revision,
            backswing,
        });
        let subject = Self::slow_subject(
            caster,
            pending.alias,
            revision,
            Phase::Accepted,
            Some(pending.target),
        );
        self.commit_ability_events(channel, subject);
        self.commit_ability_events(cast, subject);
        self.settle_slow_event(subject.slow);
    }

    pub(super) fn slow_effect_in_progress(&self, caster: HandleId) -> bool {
        self.curses
            .as_ref()
            .and_then(|s| s.pending.get(&caster))
            .is_some_and(|c| c.kind == Kind::Slow && matches!(c.stage, Stage::SlowEffect))
    }

    pub(crate) fn bound_slow_alias(&self, alias: [u8; 4]) -> bool {
        self.curses.as_ref().is_some_and(|s| {
            s.orders
                .values()
                .flatten()
                .any(|(a, k)| *a == alias && *k == Kind::Slow)
        })
    }

    pub(crate) fn withdraw_slow_ability(&mut self, caster: HandleId, alias: [u8; 4]) {
        if self
            .curses
            .as_ref()
            .and_then(|s| s.pending.get(&caster))
            .is_some_and(|c| c.alias == alias)
        {
            self.forget_curse_cast(caster);
        }
        if let Some(s) = &mut self.curses {
            s.ready_at.remove(&(caster, alias));
            for payment in s.slow_lifecycle.payments.values_mut() {
                if payment.caster == caster && payment.alias == alias {
                    payment.withdrawn = true;
                }
            }
        }
        self.autocast_ticks
            .retain(|(unit, ability, _, _)| *unit != caster || *ability != alias);
    }

    pub(super) fn step_slow_cast(&mut self, caster: HandleId, now: f64) -> Result<(), Error> {
        let Some(cast) = self.curses.as_ref().and_then(|s| s.pending.get(&caster)) else {
            return Ok(());
        };
        let (alias, target) = (cast.alias, cast.target);
        let Some(unit) = self.curse_caster(caster) else {
            self.forget_curse_cast(caster);
            return Ok(());
        };
        if unit.paused_since.is_some() {
            return Ok(());
        }
        match cast.stage {
            Stage::Walking => self.walk_curse_cast(caster, target, alias, Kind::Slow, now, None),
            Stage::Casting { effect_at } if now >= effect_at => self.emit_slow_effect(caster, now),
            Stage::SlowBackswing { until } if now >= until => self.finish_slow_cast(caster),
            Stage::Casting { .. }
            | Stage::SlowBackswing { .. }
            | Stage::SlowEffect
            | Stage::SlowEnding => Ok(()),
            Stage::Backswing => {
                Err(self.curse_error("Slow has an unqualified animation backswing"))
            }
        }
    }

    fn emit_slow_effect(&mut self, caster: HandleId, now: f64) -> Result<(), Error> {
        let cast = self
            .curses
            .as_ref()
            .and_then(|s| s.pending.get(&caster))
            .expect("Slow cast");
        let (alias, target, revision) = (
            cast.alias,
            cast.target,
            cast.slow.as_ref().expect("Slow phases").revision,
        );
        let Some(spell) = self.curse_spell(caster, alias, Kind::Slow)? else {
            return self.cancel_curse_cast(caster, now);
        };
        if !self.curse_eligible(caster, target, &spell)?
            || self.curse_unready(caster, alias, &spell, now)
        {
            return self.cancel_curse_cast(caster, now);
        }
        self.refuse_spell_shield_consulted(caster, target, &String::from_utf8_lossy(&alias))
            .map_err(|e| self.curse_error(e))?;
        let rows = self.slow_rows(caster, "EFFECT")?;
        self.reserve_game_event_delivery(rows.len())?;
        let duration = if self
            .executed_unit(target)?
            .abilities
            .as_ref()
            .is_some_and(|a| a.hero.is_some())
        {
            spell.hero_duration
        } else {
            spell.duration
        };
        let rank = self
            .executed_unit(caster)?
            .abilities
            .as_ref()
            .expect("Slow ability")
            .level(alias);
        let Effect::Slow { movement, attack } = spell.effect else {
            return Err(self.curse_error("Slow effect class differs"));
        };
        let minimum = self.slow_mana_minimum()?;
        let state = self.curses.as_mut().expect("curse admission");
        state.slow_lifecycle.payments.insert(
            revision,
            Payment {
                caster,
                target,
                alias,
                rank,
                cost: spell.cost,
                minimum,
                withdrawn: false,
                cooldown: spell.cooldown,
                buff: spell.buff,
                movement,
                attack,
                expires_at: now + duration,
            },
        );
        state.pending.get_mut(&caster).expect("Slow cast").stage = Stage::SlowEffect;
        let subject = Self::slow_subject(caster, alias, revision, Phase::Effect, Some(target));
        self.commit_ability_events(rows, subject);
        self.settle_slow_event(subject.slow);
        Ok(())
    }

    fn finish_slow_cast(&mut self, caster: HandleId) -> Result<(), Error> {
        let finish = self.slow_rows(caster, "FINISH")?;
        let end = self.slow_rows(caster, "ENDCAST")?;
        self.reserve_game_event_delivery(finish.len() + end.len())?;
        let cast = self
            .curses
            .as_mut()
            .expect("curse admission")
            .pending
            .get_mut(&caster)
            .expect("Slow cast");
        let subject = Self::slow_subject(
            caster,
            cast.alias,
            cast.slow.as_ref().expect("Slow phases").revision,
            Phase::End,
            None,
        );
        cast.stage = Stage::SlowEnding;
        self.commit_ability_events(finish, subject);
        self.commit_ability_events(end, subject);
        self.settle_slow_event(subject.slow);
        Ok(())
    }

    pub(super) fn end_interrupted_slow(&mut self, caster: HandleId) {
        let Some(cast) = self.curses.as_ref().and_then(|s| s.pending.get(&caster)) else {
            return;
        };
        let Some(slow) = &cast.slow else { return };
        if matches!(cast.stage, Stage::SlowEnding) {
            return;
        }
        let target = (!matches!(cast.stage, Stage::SlowBackswing { .. })).then_some(cast.target);
        let subject = Self::slow_subject(caster, cast.alias, slow.revision, Phase::End, target);
        let result = self.slow_rows(caster, "ENDCAST").and_then(|rows| {
            self.reserve_game_event_delivery(rows.len())?;
            self.commit_ability_events(rows, subject);
            Ok(())
        });
        if let Err(error) = result {
            self.record_slow_fault(error)
        }
    }

    fn record_slow_fault(&mut self, error: Error) {
        if let Some(state) = &mut self.curses {
            state.slow_lifecycle.fault = Some(error.to_string())
        }
    }

    pub(crate) fn settle_slow_event(&mut self, context: Option<Context>) {
        if self
            .curses
            .as_ref()
            .is_some_and(|s| s.slow_lifecycle.fault.is_some())
        {
            return;
        }
        let Some(context) = context else { return };
        if self.slow_event_pending(context) {
            return;
        }
        if let Err(error) = self.settle_slow_phase(context) {
            self.record_slow_fault(error)
        }
    }

    fn settle_slow_phase(&mut self, context: Context) -> Result<(), Error> {
        let active = self
            .curses
            .as_ref()
            .and_then(|s| s.pending.get(&context.caster))
            .filter(|c| {
                c.slow
                    .as_ref()
                    .is_some_and(|s| s.revision == context.revision)
            });
        match context.phase {
            Phase::Accepted => {
                if active.is_some() {
                    self.step_slow_cast(
                        context.caster,
                        self.timer_game_time()
                            .ok_or_else(|| self.curse_error("Slow clock missing"))?,
                    )?
                }
            }
            Phase::End => {
                if active.is_some_and(|c| matches!(c.stage, Stage::SlowEnding)) {
                    let animation = self
                        .curses
                        .as_mut()
                        .expect("curse admission")
                        .pending
                        .remove(&context.caster)
                        .and_then(|c| c.animation);
                    self.reset_cast_animation(
                        context.caster,
                        animation,
                        self.timer_game_time()
                            .ok_or_else(|| self.curse_error("Slow clock missing"))?,
                    )?;
                }
            }
            Phase::Effect => self.settle_slow_payment(context, active.is_some())?,
        }
        Ok(())
    }

    fn settle_slow_payment(&mut self, context: Context, active: bool) -> Result<(), Error> {
        let Some(payment) = self
            .curses
            .as_ref()
            .expect("curse admission")
            .slow_lifecycle
            .payments
            .get(&context.revision)
            .cloned()
        else {
            return Ok(());
        };
        if matches!(self.inner.entry(payment.caster)?, Entry::RemovedUnit { .. }) {
            self.curses
                .as_mut()
                .expect("curse admission")
                .slow_lifecycle
                .payments
                .remove(&context.revision);
            return Ok(());
        }
        let record = self.executed_unit(payment.caster)?;
        let rank = record
            .abilities
            .as_ref()
            .map_or(0, |a| a.level(payment.alias));
        if (rank != payment.rank && rank != 0) || (payment.withdrawn && rank != 0) {
            return Err(self.curse_error(
                "Slow ability changed during EFFECT; its native cost read point is unqualified",
            ));
        }
        let mana = record
            .vitals
            .ok_or_else(|| self.curse_error("Slow caster vitality missing after EFFECT"))?
            .mana;
        let remaining = f32::from_bits(
            crate::script_random::real::sub(mana.to_bits(), payment.cost.to_bits())
                .map_err(|e| self.curse_error(format!("Slow mana subtraction: {e}")))?,
        )
        .max(payment.minimum);
        let completion = if active {
            let now = self
                .timer_game_time()
                .ok_or_else(|| self.curse_error("Slow clock missing"))?;
            let cast = self
                .curses
                .as_ref()
                .expect("curse admission")
                .pending
                .get(&payment.caster)
                .ok_or_else(|| self.curse_error("active Slow cast missing"))?;
            let backswing = cast
                .slow
                .as_ref()
                .ok_or_else(|| self.curse_error("Slow phases missing"))?
                .backswing;
            Some((now + payment.cooldown, now + backswing))
        } else {
            None
        };
        if active && self.live(payment.target).is_some() {
            self.apply_slow(
                payment.target,
                payment.buff,
                payment.movement,
                payment.attack,
                payment.expires_at,
            )?;
        }
        if let Entry::Unit { record } = self.inner.entry_mut(payment.caster)? {
            record
                .vitals
                .as_mut()
                .expect("validated Slow vitality")
                .mana = remaining;
        }
        if let Some((ready_at, until)) = completion {
            let state = self.curses.as_mut().expect("curse admission");
            state
                .ready_at
                .insert((payment.caster, payment.alias), ready_at);
            let cast = state
                .pending
                .get_mut(&payment.caster)
                .expect("active Slow cast");
            cast.stage = Stage::SlowBackswing { until };
        }
        self.curses
            .as_mut()
            .expect("curse admission")
            .slow_lifecycle
            .payments
            .remove(&context.revision);
        Ok(())
    }

    pub(crate) fn next_slow_boundary(&self, until: f64) -> Option<f64> {
        let now = self.timer_game_time()?;
        self.curses
            .as_ref()?
            .pending
            .iter()
            .filter_map(|(u, c)| {
                if c.kind != Kind::Slow || self.executed_unit(*u).ok()?.paused_since.is_some() {
                    return None;
                }
                match c.stage {
                    Stage::Casting { effect_at } => Some(effect_at.max(now)),
                    Stage::SlowBackswing { until } => Some(until.max(now)),
                    _ => None,
                }
            })
            .filter(|t| *t <= until)
            .min_by(f64::total_cmp)
    }

    pub(crate) fn begin_synchronous_slow_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, Error> {
        let Some(Value::Handle(caster)) = args.first() else {
            return Ok(None);
        };
        let active = self
            .curses
            .as_ref()
            .and_then(|s| s.pending.get(caster))
            .is_some_and(|c| c.kind == Kind::Slow);
        let order = match (name, args) {
            ("IssueTargetOrder", [_, Value::Str(text), _]) => {
                self.order_registry.as_ref().and_then(|r| r.id(text))
            }
            ("IssueTargetOrderById", [_, Value::Int(order), _]) => Some(*order),
            _ => None,
        };
        let new_slow = if let Some(order) = order {
            self.curse_alias_for_order(*caster, order)?
                .is_some_and(|(_, k)| k == Kind::Slow)
        } else {
            false
        };
        let interrupts = matches!(
            name,
            "IssueTargetOrder"
                | "IssueTargetOrderById"
                | "IssueImmediateOrder"
                | "IssueImmediateOrderById"
                | "IssuePointOrder"
                | "IssuePointOrderById"
                | "IssuePointOrderLoc"
                | "IssuePointOrderByIdLoc"
                | "KillUnit"
                | "RemoveUnit"
                | "UnitRemoveAbility"
        );
        if !new_slow && !(active && interrupts) {
            return Ok(None);
        }
        self.prepare_synchronous_frame(name)?;
        self.begin_synchronous_delivery();
        let result = self.native(name, args);
        let events = self
            .take_synchronous_delivery()
            .ok_or_else(|| self.curse_error("Slow synchronous batch missing"))?;
        let result = result.map_err(|e| self.curse_error(e))?;
        self.begin_synchronous_events_with_result(events, result)
            .map(Some)
    }
}

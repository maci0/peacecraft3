//! Mirror identities share the ordinary unit arena and prepared creation path.
use super::{Error, Failure, ObjectHost};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::{HandleId, RealOps, Value, ast::BinOp};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MirrorEventPhase {
    Accepted,
    Effect,
    End,
    Image,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MirrorEventContext {
    pub caster: HandleId,
    pub revision: u64,
    pub phase: MirrorEventPhase,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Windup,
    Effect,
    End,
    Separate,
    Reveal,
    Cancelled,
}
struct Pending {
    revision: u64,
    phase: Phase,
    due: f64,
    restore: bool,
    reveal: Option<super::unit_visibility::PreparedVisibility>,
}
pub(super) struct State {
    rules: Arc<crate::mirror_image_rules::Rules>,
    pub(super) ready: BTreeMap<HandleId, f64>,
    pending: BTreeMap<HandleId, Pending>,
    revision: u64,
    fault: Option<String>,
}
#[derive(Clone)]
struct Spec {
    ability: [u8; 4],
    cost: f32,
    cooldown: f64,
    duration: f64,
    delay: f64,
    windup: f64,
    count: usize,
    radius: f32,
    factors: [f32; 2],
}

/// Effective combat snapshots remain distinct from public hero identity.
#[derive(Clone, Debug, PartialEq)]
pub struct Illusion {
    pub caster: HandleId,
    pub ability: [u8; 4],
    pub dealt_factor: f32,
    pub taken_factor: f32,
    pub attributes: super::combat::HeroAttributeStat,
    pub(crate) combat_bonuses: super::combat::HeroBonuses,
}

impl ObjectHost {
    pub(super) fn illusion_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != "IsUnitIllusion" || self.units.is_none() {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some((|| {
            Self::contract(&self.inner, name, &["unit"], "boolean", true, Some("unit"))?;
            let value = match args {
                [Value::Null] => false,
                [Value::Handle(unit)] => match self.inner.entry(*unit)? {
                    super::Entry::RemovedUnit { .. } => false,
                    super::Entry::Unit { record } => record.illusion.is_some(),
                    _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                },
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            Ok(Value::Bool(value))
        })())
    }

    #[cfg(test)]
    pub(crate) fn test_mirror_image_factory(
        &mut self,
        caster: HandleId,
        ability: [u8; 4],
        point: [f32; 2],
        factors: [f32; 2],
        commit: bool,
    ) -> Result<Option<HandleId>, Error> {
        let prepared = self.prepare_mirror_image(caster, ability, point, factors)?;
        if commit {
            self.commit_unit_creation(prepared).map(Some)
        } else {
            Ok(None)
        }
    }

    // Creation remains internal to the canonical cast scheduler; tests may
    // exercise the snapshot independently of the public command surface.
    pub(super) fn prepare_mirror_image(
        &mut self,
        caster: HandleId,
        ability: [u8; 4],
        point: [f32; 2],
        factors: [f32; 2],
    ) -> Result<super::unit_creation::PreparedUnit, Error> {
        let fail = |detail: &str| {
            self.inner
                .error("Mirror Image", Failure::UnitActivity(detail.into()))
        };
        if !point.into_iter().all(f32::is_finite)
            || !factors.into_iter().all(|v| v.is_finite() && v >= 0.0)
        {
            return Err(fail("invalid image placement or damage factor"));
        }
        let source = self.executed_unit(caster)?;
        if source.illusion.is_some()
            || source.construction.is_some()
            || source.combat.as_ref().is_none_or(|c| c.dead)
            || source.vitals.is_none_or(|v| v.life <= 0.0)
        {
            return Err(fail("image snapshot requires a live ordinary caster"));
        }
        let abilities = source
            .abilities
            .as_ref()
            .ok_or_else(|| fail("caster ability snapshot missing"))?;
        if abilities.hero.is_none() || abilities.level(ability) <= 0 {
            return Err(fail("image snapshot requires the learned hero ability"));
        }
        if !abilities.cooldowns().is_empty() {
            return Err(fail(
                "image snapshot of custom cooldown overrides is not qualified",
            ));
        }
        if self
            .executed_items()
            .any(|(_, item)| item.held_by.is_some_and(|(owner, _)| owner == caster))
        {
            return Err(fail("image snapshot of held inventory is not qualified"));
        }
        let combat_bonuses = self.hero_combat_bonuses(caster)?;
        let attributes = self
            .unit_stat_snapshot(caster)?
            .attributes
            .ok_or_else(|| fail("caster effective attributes missing"))?;
        let (owner, rawcode, face, abilities, vitals, combat, permanent_strength, health_handicap) = (
            source.owner,
            source.definition.rawcode,
            source.face,
            abilities.mirror_snapshot(),
            source.vitals,
            source.combat.clone().expect("checked combat"),
            source.permanent_strength,
            source.health_handicap,
        );
        let mut prepared = self.prepare_unit_creation(
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(rawcode)),
                Value::Real(point[0]),
                Value::Real(point[1]),
                Value::Real(face),
            ],
            None,
            super::unit_creation::Birth::Illusion,
        )?;
        let record = &mut prepared.record;
        record.illusion = Some(Illusion {
            caster,
            ability,
            dealt_factor: factors[0],
            taken_factor: factors[1],
            attributes,
            combat_bonuses,
        });
        record.abilities = Some(abilities);
        record.vitals = vitals;
        record.permanent_strength = permanent_strength;
        record.health_handicap = health_handicap;
        let fresh_combat = record.combat.as_mut().ok_or_else(|| {
            self.inner.error(
                "Mirror Image",
                Failure::UnitActivity("image combat snapshot missing".into()),
            )
        })?;
        // Copy effective numbers, keeping the newly prepared idle combat lifecycle.
        fresh_combat.weapon = combat.weapon;
        fresh_combat.armor = combat.armor;
        fresh_combat.defense = combat.defense;
        Ok(prepared)
    }
}

impl ObjectHost {
    fn mirror_error(&self, reason: impl Into<String>) -> Error {
        self.inner
            .error("Mirror Image", Failure::UnitActivity(reason.into()))
    }
    pub fn admit_mirror_image(&mut self) -> Result<(), Error> {
        let Some(Ok(rules)) = self.admitted_mirror_rules() else {
            return Ok(());
        };
        if self.mirror_image.is_some() {
            return Err(self.mirror_error("already admitted"));
        }
        if self
            .numeric
            .as_ref()
            .is_none_or(|n| n.image_sha256() != &rules.image_sha256)
        {
            return Err(self.mirror_error("numeric/source identity differs"));
        }
        let profile = self
            .hero_ability_profile(rules.ability, "Order")
            .map_err(|e| self.mirror_error(e))?;
        if self.order_registry.as_ref().and_then(|r| r.id(&profile)) != Some(rules.order) {
            return Err(self.mirror_error("profile and native order differ"));
        }
        for suffix in ["CHANNEL", "CAST", "EFFECT", "ENDCAST"] {
            self.source_constant_enumerand(&format!("EVENT_UNIT_SPELL_{suffix}"), "unitevent")?;
            self.source_constant_enumerand(
                &format!("EVENT_PLAYER_UNIT_SPELL_{suffix}"),
                "playerunitevent",
            )?;
        }
        self.mirror_image = Some(State {
            rules,
            ready: BTreeMap::new(),
            pending: BTreeMap::new(),
            revision: 0,
            fault: None,
        });
        Ok(())
    }
    fn mirror_math(&self, op: BinOp, a: f32, b: f32) -> Result<f32, Error> {
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| self.mirror_error("numeric policy missing"))?;
        match numeric
            .binary(op, a, b)
            .map_err(|e| self.mirror_error(e.to_string()))?
        {
            Value::Real(v) if v.is_finite() => Ok(v),
            _ => Err(self.mirror_error("nonfinite arithmetic")),
        }
    }
    pub(super) fn mirror_alias(&self, alias: [u8; 4]) -> bool {
        self.mirror_image
            .as_ref()
            .is_some_and(|s| self.ability_parent_code(alias).ok() == Some(s.rules.ability))
    }
    fn mirror_spec(&self, caster: HandleId) -> Result<Option<Spec>, Error> {
        let Some(state) = &self.mirror_image else {
            return Ok(None);
        };
        let record = match self.executed_unit(caster) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if record.illusion.is_some()
            || record.combat.as_ref().is_none_or(|c| c.dead)
            || record.vitals.is_none_or(|v| v.life <= 0.)
        {
            return Ok(None);
        }
        let Some(abilities) = &record.abilities else {
            return Ok(None);
        };
        let aliases = abilities
            .levels()
            .filter(|(_, rank)| *rank > 0)
            .filter_map(|(alias, rank)| self.mirror_alias(alias).then_some((alias, rank)))
            .collect::<Vec<_>>();
        let [(ability, rank)] = aliases.as_slice() else {
            if aliases.is_empty() {
                return Ok(None);
            }
            return Err(self.mirror_error("multiple learned Mirror classes are ambiguous"));
        };
        if !self.unit_ability_available(caster, *ability)? {
            return Ok(None);
        }
        if *rank
            > self
                .hero_ability_number(*ability, "levels")
                .map_err(|e| self.mirror_error(e))?
        {
            return Err(self.mirror_error("rank exceeds source levels"));
        }
        let text = |stem, kind| {
            self.hero_ability_level_cell(*ability, stem, *rank, kind)
                .map_err(|e| self.mirror_error(e))
        };
        let number = |stem, kind| -> Result<f32, Error> {
            let n = text(stem, kind)?
                .parse::<f32>()
                .map_err(|_| self.mirror_error(format!("invalid {stem} rank {rank}")))?;
            if !n.is_finite() || n < 0. {
                return Err(self.mirror_error(format!("invalid {stem} rank {rank}")));
            }
            Ok(n)
        };
        let count = text("DataA", FieldKind::Int)?
            .parse::<usize>()
            .map_err(|_| self.mirror_error("invalid image count"))?;
        let duration = number("HeroDur", FieldKind::Unreal)?;
        if count == 0 || count > state.rules.image_limit || duration <= 0. {
            return Err(self.mirror_error("image count/lifetime outside admitted source shape"));
        }
        let buff = text("BuffID", FieldKind::Token)?;
        if buff.as_bytes() != state.rules.buff {
            return Err(self.mirror_error("ranked buff differs from admitted Mirror class"));
        }
        let windup = super::combat::column(&record.definition, SlkId::UnitWeapons, "castpt")
            .map_err(|e| self.mirror_error(e))?
            .ok_or_else(|| self.mirror_error("cast point missing"))?;
        let windup = super::combat::number(windup, "castpt")
            .map_err(|e| self.mirror_error(e))?
            .ok_or_else(|| self.mirror_error("cast point empty"))?;
        if !windup.is_finite() || windup < 0. {
            return Err(self.mirror_error("invalid cast point"));
        }
        Ok(Some(Spec {
            ability: *ability,
            cost: number("Cost", FieldKind::Int)?,
            cooldown: f64::from(self.ability_cooldown_duration(caster, *ability, *rank)?),
            duration: f64::from(duration),
            delay: f64::from(number("DataD", FieldKind::Unreal)?),
            windup: f64::from(windup),
            count,
            radius: number("Rng", FieldKind::Unreal)?,
            factors: [
                number("DataB", FieldKind::Unreal)?,
                number("DataC", FieldKind::Unreal)?,
            ],
        }))
    }
    fn mirror_points(&self, caster: HandleId, spec: &Spec) -> Result<Vec<[f32; 2]>, Error> {
        let record = self.executed_unit(caster)?;
        let origin = record
            .position
            .ok_or_else(|| self.mirror_error("accepted caster position missing"))?;
        let count = spec
            .count
            .checked_add(1)
            .ok_or_else(|| self.mirror_error("circle count overflow"))?;
        let mut points = Vec::new();
        points
            .try_reserve_exact(count)
            .map_err(|_| self.mirror_error("circle allocation"))?;
        let full = self
            .mirror_image
            .as_ref()
            .expect("admitted")
            .rules
            .circle_radians;
        for i in 0..count {
            let angle = self.mirror_math(
                BinOp::Div,
                self.mirror_math(BinOp::Mul, full, i as f32)?,
                count as f32,
            )?;
            // shortcut: replace host trigonometry/heading conversion after native numeric and transform rounding are qualified.
            let angle = angle + record.face.to_radians();
            let (sin, cos) = angle.sin_cos();
            points.push([
                self.mirror_math(
                    BinOp::Add,
                    origin[0],
                    self.mirror_math(BinOp::Mul, cos, spec.radius)?,
                )?,
                self.mirror_math(
                    BinOp::Add,
                    origin[1],
                    self.mirror_math(BinOp::Mul, sin, spec.radius)?,
                )?,
            ]);
        }
        Ok(points)
    }
    fn preflight_mirror(&mut self, caster: HandleId, spec: &Spec) -> Result<(), Error> {
        self.check_unit_creation_capacity(spec.count)?;
        for point in self.mirror_points(caster, spec)? {
            let _ = self.prepare_mirror_image(caster, spec.ability, point, spec.factors)?;
        }
        self.mirror_math(
            BinOp::Sub,
            self.executed_unit(caster)?
                .vitals
                .expect("factory checked")
                .mana,
            spec.cost,
        )?;
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.mirror_error("clock missing"))?;
        if ![spec.windup, spec.delay, spec.duration, spec.cooldown]
            .into_iter()
            .all(|v| (now + v).is_finite())
        {
            return Err(self.mirror_error("time overflow"));
        }
        Ok(())
    }
    pub(super) fn mirror_damage(
        &self,
        source: HandleId,
        target: HandleId,
        mut damage: f32,
    ) -> Result<f32, Error> {
        if let Some(image) = self.executed_unit(source)?.illusion.as_ref() {
            damage = self.mirror_math(BinOp::Mul, damage, image.dealt_factor)?;
        }
        if let Some(image) = self.executed_unit(target)?.illusion.as_ref() {
            damage = self.mirror_math(BinOp::Mul, damage, image.taken_factor)?;
        }
        Ok(damage)
    }
    pub(super) fn mirror_current_order(&self, unit: HandleId) -> Option<i32> {
        let state = self.mirror_image.as_ref()?;
        let job = state.pending.get(&unit)?;
        matches!(job.phase, Phase::Windup | Phase::Effect | Phase::End).then_some(state.rules.order)
    }
    pub(super) fn mirror_protects(&self, unit: HandleId) -> bool {
        self.mirror_image
            .as_ref()
            .and_then(|s| s.pending.get(&unit))
            .is_some_and(|j| matches!(j.phase, Phase::Separate | Phase::Reveal))
    }
    pub(super) fn mirror_busy(&self, unit: HandleId) -> bool {
        self.mirror_image
            .as_ref()
            .is_some_and(|s| s.pending.contains_key(&unit))
    }
    pub(super) fn cancel_mirror_cast(&mut self, unit: HandleId) {
        let now = self.timer_game_time().unwrap_or(0.);
        if let Some(job) = self
            .mirror_image
            .as_mut()
            .and_then(|s| s.pending.get_mut(&unit))
        {
            job.phase = Phase::Cancelled;
            job.due = now;
        }
    }
    pub(super) fn forget_mirror_unit(&mut self, unit: HandleId) {
        if let Some(s) = &mut self.mirror_image {
            s.pending.remove(&unit);
            s.ready.remove(&unit);
        }
    }
    pub(super) fn mirror_visibility_override(&mut self, unit: HandleId) {
        if let Some(job) = self
            .mirror_image
            .as_mut()
            .and_then(|s| s.pending.get_mut(&unit))
        {
            job.restore = false;
        }
    }
    pub(super) fn resume_mirror_cast(&mut self, unit: HandleId, elapsed: f64) {
        if let Some(job) = self
            .mirror_image
            .as_mut()
            .and_then(|s| s.pending.get_mut(&unit))
        {
            job.due += elapsed;
        }
    }
    pub(crate) fn next_mirror_boundary(&self, until: f64) -> Option<f64> {
        let now = self.timer_game_time()?;
        self.mirror_image
            .as_ref()?
            .pending
            .iter()
            .filter_map(|(u, j)| {
                let r = self.executed_unit(*u).ok()?;
                (r.paused_since.is_none()
                    && matches!(j.phase, Phase::Windup | Phase::Separate | Phase::Cancelled))
                .then_some(j.due.max(now))
            })
            .filter(|t| *t <= until)
            .min_by(f64::total_cmp)
    }
}

impl ObjectHost {
    fn mirror_event_rows(
        &mut self,
        caster: HandleId,
        suffix: &str,
    ) -> Result<Vec<super::combat::AbilityEvent>, Error> {
        self.prepare_ability_events(
            caster,
            &format!("EVENT_UNIT_SPELL_{suffix}"),
            &format!("EVENT_PLAYER_UNIT_SPELL_{suffix}"),
        )
    }
    fn mirror_subject(
        caster: HandleId,
        ability: [u8; 4],
        revision: u64,
        phase: MirrorEventPhase,
    ) -> super::EventSubject {
        super::EventSubject {
            unit: Some(caster),
            spell_ability: Some(i32::from_be_bytes(ability)),
            mirror: Some(MirrorEventContext {
                caster,
                revision,
                phase,
            }),
            ..Default::default()
        }
    }
    fn mirror_order(&self, name: &str, args: &[Value]) -> Option<i32> {
        let state = self.mirror_image.as_ref()?;
        let [_, order] = args else { return None };
        let order = match (name, order) {
            ("IssueImmediateOrder", Value::Str(s)) => self.order_registry.as_ref()?.id(s)?,
            ("IssueImmediateOrderById", Value::Int(i)) => *i,
            _ => return None,
        };
        (order == state.rules.order).then_some(order)
    }
    pub(super) fn mirror_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.mirror_order(name, args)?;
        Some(self.accept_mirror(args).map(Value::Bool))
    }
    fn accept_mirror(&mut self, args: &[Value]) -> Result<bool, Error> {
        let [unit, _] = args else {
            return Err(self.mirror_error("unit and order required"));
        };
        let caster = match unit {
            Value::Null => return Ok(false),
            Value::Handle(u) => *u,
            _ => return Err(self.mirror_error("unit required")),
        };
        let Some(spec) = self.mirror_spec(caster)? else {
            return Ok(false);
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.mirror_error("clock missing"))?;
        let record = self.executed_unit(caster)?;
        if !record.shown
            || self.mirror_busy(caster)
            || !Self::ability_readiness_at(
                record,
                spec.cost,
                self.mirror_image
                    .as_ref()
                    .and_then(|s| s.ready.get(&caster).copied()),
                now,
            )
            .ready()
        {
            return Ok(false);
        }
        self.preflight_mirror(caster, &spec)?;
        let channel = self.mirror_event_rows(caster, "CHANNEL")?;
        let cast = self.mirror_event_rows(caster, "CAST")?;
        self.reserve_game_event_delivery(
            channel
                .len()
                .checked_add(cast.len())
                .ok_or_else(|| self.mirror_error("event count overflow"))?,
        )?;
        let revision = self
            .mirror_image
            .as_ref()
            .expect("admitted")
            .revision
            .checked_add(1)
            .ok_or_else(|| self.mirror_error("cast revision exhausted"))?;
        let mut animation = self
            .executed_unit(caster)?
            .animation
            .clone()
            .filter(|a| a.has_sequence("spell"));
        if let Some(a) = &mut animation {
            a.select("spell", now, None)
                .map_err(|e| self.mirror_error(e))?;
        }
        self.steer_unit(caster, None, "Mirror Image")?;
        self.unit_ordered(caster);
        self.cancel_inventory_order(caster);
        self.clear_combat_target(caster);
        if let super::Entry::Unit { record } = self.inner.entry_mut(caster)?
            && animation.is_some()
        {
            record.animation = animation;
        }
        let state = self.mirror_image.as_mut().expect("admitted");
        state.revision = revision;
        state.pending.insert(
            caster,
            Pending {
                revision,
                phase: Phase::Windup,
                due: now + spec.windup,
                restore: true,
                reveal: None,
            },
        );
        let subject =
            Self::mirror_subject(caster, spec.ability, revision, MirrorEventPhase::Accepted);
        self.commit_ability_events(channel, subject);
        self.commit_ability_events(cast, subject);
        Ok(true)
    }
    pub(super) fn begin_synchronous_mirror_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<pcx_script::ScriptCall>, Error> {
        if self.mirror_order(name, args).is_none() {
            return Ok(None);
        }
        self.prepare_synchronous_frame(name)?;
        self.begin_synchronous_delivery();
        let result = self.accept_mirror(args);
        let events = self
            .take_synchronous_delivery()
            .ok_or_else(|| self.mirror_error("synchronous batch missing"))?;
        let accepted = result?;
        self.begin_synchronous_events_with_result(events, Value::Bool(accepted))
            .map(Some)
    }
    pub(super) fn settle_mirror_event(&mut self, context: Option<MirrorEventContext>) {
        let Some(context) = context else { return };
        if self.mirror_event_pending(context) {
            return;
        }
        let result = self.settle_mirror_phase(context);
        if let Err(e) = result
            && let Some(s) = &mut self.mirror_image
        {
            s.fault = Some(e.to_string());
        }
    }
    fn settle_mirror_phase(&mut self, c: MirrorEventContext) -> Result<(), Error> {
        let Some(job) = self
            .mirror_image
            .as_ref()
            .and_then(|s| s.pending.get(&c.caster))
        else {
            return Ok(());
        };
        if job.revision != c.revision {
            return Ok(());
        }
        let expected = match c.phase {
            MirrorEventPhase::Accepted => return Ok(()),
            MirrorEventPhase::Effect => Phase::Effect,
            MirrorEventPhase::End => Phase::End,
            MirrorEventPhase::Image => Phase::Reveal,
        };
        if job.phase != expected {
            return Ok(());
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.mirror_error("clock missing"))?;
        // Summon callbacks may change ability availability. Restoration belongs
        // to the committed cast identity, independently of its current rank.
        if c.phase == MirrorEventPhase::Image {
            let job = self
                .mirror_image
                .as_mut()
                .expect("admitted")
                .pending
                .remove(&c.caster)
                .expect("job");
            if job.restore
                && let Some(reveal) = job.reveal
            {
                self.commit_unit_visibility(c.caster, reveal, "Mirror Image reveal")?;
            }
            self.finish_mirror_animation(c.caster, now)?;
            return Ok(());
        }
        let Some(spec) = self.mirror_spec(c.caster)? else {
            self.cancel_mirror_cast(c.caster);
            return Ok(());
        };
        match c.phase {
            MirrorEventPhase::Effect => {
                let mana = self
                    .executed_unit(c.caster)?
                    .vitals
                    .expect("preflight")
                    .mana;
                if mana < spec.cost {
                    self.cancel_mirror_cast(c.caster);
                    return Ok(());
                }
                let debit = self.mirror_math(BinOp::Sub, mana, spec.cost)?;
                let events = self.mirror_event_rows(c.caster, "ENDCAST")?;
                self.reserve_game_event_delivery(events.len())?;
                if let super::Entry::Unit { record } = self.inner.entry_mut(c.caster)? {
                    record.vitals.as_mut().expect("checked").mana = debit;
                }
                let s = self.mirror_image.as_mut().expect("admitted");
                s.ready.insert(c.caster, now + spec.cooldown);
                s.pending.get_mut(&c.caster).expect("job").phase = Phase::End;
                let subject =
                    Self::mirror_subject(c.caster, spec.ability, c.revision, MirrorEventPhase::End);
                self.commit_ability_events(events, subject);
                self.settle_mirror_event(subject.mirror);
            }
            MirrorEventPhase::End => {
                let hide =
                    self.prepare_unit_visibility(c.caster, false, "Mirror Image separate")?;
                self.commit_unit_visibility(c.caster, hide, "Mirror Image separate")?;
                let job = self
                    .mirror_image
                    .as_mut()
                    .expect("admitted")
                    .pending
                    .get_mut(&c.caster)
                    .expect("job");
                job.phase = Phase::Separate;
                job.due = now + spec.delay;
            }
            MirrorEventPhase::Image => unreachable!("settled above"),
            MirrorEventPhase::Accepted => {}
        }
        Ok(())
    }
    fn finish_mirror_animation(&mut self, caster: HandleId, now: f64) -> Result<(), Error> {
        if let Ok(super::Entry::Unit { record }) = self.inner.entry_mut(caster)
            && let Some(a) = &mut record.animation
            && a.has_sequence("stand")
            && a.sequence.is_some_and(|i| {
                a.model.sequences[i]
                    .name
                    .to_ascii_lowercase()
                    .contains("spell")
            })
        {
            a.select("stand", now, None)
                .map_err(|e| self.inner.error("Mirror Image", Failure::UnitAnimation(e)))?;
        }
        Ok(())
    }
    pub(super) fn advance_mirror_image(&mut self, now: f64) -> Result<(), Error> {
        let Some(state) = &self.mirror_image else {
            return Ok(());
        };
        if let Some(fault) = &state.fault {
            return Err(self.mirror_error(fault.clone()));
        }
        let due = state
            .pending
            .iter()
            .filter(|(_, j)| {
                j.due <= now
                    && matches!(j.phase, Phase::Windup | Phase::Separate | Phase::Cancelled)
            })
            .map(|(u, _)| *u)
            .collect::<Vec<_>>();
        for caster in due {
            let Some(record) = self
                .executed_unit(caster)
                .ok()
                .filter(|r| r.combat.as_ref().is_some_and(|c| !c.dead))
            else {
                self.forget_mirror_unit(caster);
                continue;
            };
            if record.paused_since.is_some() {
                continue;
            }
            let job = self
                .mirror_image
                .as_ref()
                .expect("admitted")
                .pending
                .get(&caster)
                .expect("due");
            let (phase, revision, restore) = (job.phase, job.revision, job.restore);
            if phase == Phase::Cancelled {
                if restore && !record.shown {
                    let reveal =
                        self.prepare_unit_visibility(caster, true, "Mirror Image cancel")?;
                    self.commit_unit_visibility(caster, reveal, "Mirror Image cancel")?;
                }
                self.mirror_image
                    .as_mut()
                    .expect("admitted")
                    .pending
                    .remove(&caster);
                self.finish_mirror_animation(caster, now)?;
                continue;
            }
            let Some(spec) = self.mirror_spec(caster)? else {
                self.cancel_mirror_cast(caster);
                continue;
            };
            if phase == Phase::Windup {
                if !record.shown {
                    self.cancel_mirror_cast(caster);
                    continue;
                }
                self.preflight_mirror(caster, &spec)?;
                let events = self.mirror_event_rows(caster, "EFFECT")?;
                self.reserve_game_event_delivery(events.len())?;
                self.mirror_image
                    .as_mut()
                    .expect("admitted")
                    .pending
                    .get_mut(&caster)
                    .expect("due")
                    .phase = Phase::Effect;
                let subject =
                    Self::mirror_subject(caster, spec.ability, revision, MirrorEventPhase::Effect);
                self.commit_ability_events(events, subject);
                self.settle_mirror_event(subject.mirror);
                continue;
            }
            self.preflight_mirror(caster, &spec)?;
            let mut prepared = Vec::new();
            let mut reveals = Vec::new();
            for point in self.mirror_points(caster, &spec)? {
                let image = self.prepare_mirror_image(caster, spec.ability, point, spec.factors)?;
                let accepted = image
                    .record
                    .position
                    .ok_or_else(|| self.mirror_error("image position missing"))?;
                let reveal = if restore {
                    Some(self.prepare_mirror_reveal(caster, accepted)?)
                } else {
                    None
                };
                prepared.push(image);
                reveals.push(reveal);
            }
            // A prepared batch must not publish two canonical units at the same accepted point.
            for i in 0..prepared.len() {
                for j in 0..i {
                    if prepared[i].record.position == prepared[j].record.position {
                        return Err(self.mirror_error("image placement batch overlaps"));
                    }
                }
            }
            let events = self.prepare_ability_events(
                caster,
                "EVENT_UNIT_SUMMON",
                "EVENT_PLAYER_UNIT_SUMMON",
            )?;
            self.reserve_game_event_delivery(
                events
                    .len()
                    .checked_mul(spec.count)
                    .ok_or_else(|| self.mirror_error("summon event count overflow"))?,
            )?;
            self.inner.reserve_handles(spec.count, "Mirror Image")?;
            let sides = u32::try_from(prepared.len())
                .map_err(|_| self.mirror_error("circle exceeds die range"))?;
            let channel = self
                .mirror_image
                .as_ref()
                .expect("admitted")
                .rules
                .random_channel;
            let real = self.draw_random_secondary_die(channel, sides, "Mirror Image")? as usize;
            let reveal = reveals.swap_remove(real);
            prepared.swap_remove(real);
            let mut images = Vec::with_capacity(spec.count);
            for image in prepared {
                let unit = self.commit_unit_creation(image)?;
                self.unit_expirations.insert(unit, now + spec.duration);
                images.push(unit);
            }
            let job = self
                .mirror_image
                .as_mut()
                .expect("admitted")
                .pending
                .get_mut(&caster)
                .expect("due");
            job.phase = Phase::Reveal;
            job.reveal = reveal;
            for image in images {
                self.commit_ability_events(
                    events.clone(),
                    super::EventSubject {
                        unit: Some(image),
                        summon: Some(super::SummonContext {
                            summoner: caster,
                            summoned: image,
                        }),
                        mirror: Some(MirrorEventContext {
                            caster,
                            revision,
                            phase: MirrorEventPhase::Image,
                        }),
                        ..Default::default()
                    },
                );
            }
            self.settle_mirror_event(Some(MirrorEventContext {
                caster,
                revision,
                phase: MirrorEventPhase::Image,
            }));
        }
        Ok(())
    }
}

#[cfg(test)]
impl ObjectHost {
    pub(crate) fn test_admit_mirror(&mut self) {
        let (image, binding, budget) = crate::script_numeric::tests::fixture();
        let numeric =
            crate::script_numeric::NumericPolicy::read(&image, binding, budget, 64).unwrap();
        let rules = crate::mirror_image_rules::Rules {
            origin: crate::scripts::AssetIdentity {
                source: "fixture".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            image_sha256: *numeric.image_sha256(),
            ability: *b"AHwe",
            buff: *b"Zbuf",
            order: self.order_registry.as_ref().unwrap().id("gamma").unwrap(),
            image_limit: 3,
            random_channel: 1,
            circle_radians: std::f32::consts::TAU,
        };
        self.numeric = Some(numeric);
        self.mirror_image = Some(State {
            rules: Arc::new(rules),
            ready: BTreeMap::new(),
            pending: BTreeMap::new(),
            revision: 0,
            fault: None,
        });
    }
}

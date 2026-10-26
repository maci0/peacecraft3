//! Shared producer queues and guard replacement through canonical unit creation.
use super::{Entry, Error, EventSubject, ExecutedUnit, Failure, ObjectHost};
use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
use pcx_script::{HandleId, Value, value::FunctionId};
use pcx_sim::world::{Definition, FieldProblem, FieldValue};

const NATIVE: &str = "FillGuardPosts";

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct GuardPost {
    kind: i32,
    home: [f32; 2],
    unit: Option<HandleId>,
    assignments: i32,
}

#[derive(Clone, Copy)]
struct Training {
    producer: HandleId,
    owner: HandleId,
    guard: Option<(i32, usize)>,
    active: bool,
    duration: i32,
    gold: i32,
    lumber: i32,
    kind: i32,
    food: i32,
    finish: f64,
    through: f64,
    pause: Option<(f64, f64)>,
}

#[derive(Default)]
pub(super) struct State {
    jobs: Vec<Training>,
}

pub(super) struct TrainingDeath {
    producer: HandleId,
    credit: super::players::TrainingDebit,
}

pub(super) struct GuardBinding {
    player: i32,
    post: usize,
    count: i32,
    pub(super) home: [f32; 2],
    pub(super) route: super::unit_activity::PreparedSteer,
}

struct TrainingEvent {
    trigger: HandleId,
    filter: Option<FunctionId>,
    event: HandleId,
    player: Option<HandleId>,
}

fn live(record: &ExecutedUnit) -> bool {
    record.combat.as_ref().is_none_or(|c| !c.dead)
        && record
            .vitals
            .is_some_and(|v| v.life.is_finite() && v.life > 0.)
}

fn cached_without_rally(record: &ExecutedUnit) -> bool {
    record.cached
        && record
            .activity
            .as_ref()
            .is_none_or(|activity| activity.rally.is_none())
}

pub(super) fn profile<'a>(
    definition: &'a Definition,
    key: &str,
    declared: &str,
) -> Result<&'a str, String> {
    profile_value(definition, key, declared, FieldKind::Token)
}

fn profile_value<'a>(
    definition: &'a Definition,
    key: &str,
    declared: &str,
    kind: FieldKind,
) -> Result<&'a str, String> {
    let mut matches = definition.fields.values().filter(|field| {
        field.metadata.as_ref().is_some_and(
        |m| matches!(&m.source, FieldSource::Profile { key: k, .. } if k.eq_ignore_ascii_case(key))
    )
    });
    let field = matches
        .next()
        .ok_or_else(|| format!("{key} profile metadata is not admitted"))?;
    let metadata = field.metadata.as_ref().expect("matched metadata");
    if matches.next().is_some()
        || metadata.kind != kind
        || metadata.declared != declared
        || metadata.from != SlkId::UnitMetaData
        || metadata.index != -1
        || !metadata.field.eq_ignore_ascii_case(key)
        || !matches!(&metadata.source, FieldSource::Profile { index: -1, .. })
    {
        return Err(format!("{key} profile metadata is ambiguous or indexed"));
    }
    let text = match &field.value {
        Ok(FieldValue::Cell(text)) => text.as_str(),
        Ok(FieldValue::Override(pcx_objects::Value::Str(text))) => {
            std::str::from_utf8(text).map_err(|_| format!("{key} override is not UTF-8"))?
        }
        Err(FieldProblem::Missing) => "",
        _ => return Err(format!("{key} profile value is malformed")),
    };
    Ok(match text.trim() {
        "_" | "-" => "",
        text => text,
    })
}

impl ObjectHost {
    fn training_error(&self, detail: impl Into<String>) -> Error {
        self.inner
            .error(NATIVE, Failure::UnitActivity(detail.into()))
    }

    pub(super) fn training_prerequisites_met(
        &self,
        definition: &Definition,
        owner: HandleId,
    ) -> Result<bool, Error> {
        let fail = |detail| self.training_error(detail);
        let requires = profile(definition, "Requires", "techList").map_err(fail)?;
        if requires.is_empty() {
            return Ok(true);
        }
        let amounts = profile_value(definition, "Requiresamount", "intList", FieldKind::IntList)
            .map_err(fail)?;
        let mut amounts = (!amounts.is_empty()).then(|| amounts.split(','));
        let mut eligible = true;
        for token in requires.split(',') {
            let code: [u8; 4] = token
                .trim()
                .as_bytes()
                .try_into()
                .map_err(|_| fail("malformed Requires rawcode".into()))?;
            if !code.iter().all(u8::is_ascii_graphic) {
                return Err(fail("malformed Requires rawcode".into()));
            }
            let id = i32::from_be_bytes(code);
            let required = if let Some(amounts) = &mut amounts {
                let required = amounts
                    .next()
                    .ok_or_else(|| fail("Requiresamount has fewer entries than Requires".into()))?
                    .trim()
                    .parse::<i32>()
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| fail("Requiresamount must contain positive integers".into()))?;
                Some(required)
            } else {
                None
            };
            eligible &= self
                .technology_requirement_met(owner, id, required)
                .map_err(|error| {
                    fail(format!(
                        "training prerequisite {}: {error}",
                        String::from_utf8_lossy(&code)
                    ))
                })?;
        }
        if amounts.is_some_and(|mut amounts| amounts.next().is_some()) {
            return Err(fail("Requiresamount has more entries than Requires".into()));
        }
        Ok(eligible)
    }

    pub(super) fn prepare_initial_guard_posts(&self, player: i32) -> Result<Vec<GuardPost>, Error> {
        let mut posts = Vec::new();
        let (Some(policy), Some(guards)) = (&self.campaign_mode, &self.guards) else {
            return Ok(posts);
        };
        let owner = self
            .player_identity(player)
            .ok_or_else(|| self.training_error("AI owner missing"))?;
        for &(unit, guard) in guards.records() {
            if guard.owner != owner || guard.released {
                continue;
            }
            let record = self.executed_unit(unit)?;
            if !self.ordinary_post_candidate(record)? {
                continue;
            }
            if record.vitals.is_none() {
                return Err(self.training_error("guard-post liveness is not admitted"));
            }
            if !live(record) {
                continue;
            }
            if self.unit_has_effect_history(unit) {
                return Err(self.training_error("guard-post effect state is unsupported"));
            }
            posts
                .try_reserve(1)
                .map_err(|_| self.training_error("guard-post capacity"))?;
            posts.push(GuardPost {
                kind: i32::from_be_bytes(record.definition.rawcode),
                home: record
                    .position
                    .ok_or_else(|| self.training_error("guard-post position is not admitted"))?,
                unit: Some(unit),
                assignments: policy
                    .guard_posts
                    .initial_count
                    .wrapping_add(policy.guard_posts.binding_increment),
            });
        }
        Ok(posts)
    }

    fn ordinary_post_candidate(&self, record: &ExecutedUnit) -> Result<bool, Error> {
        let classifier = self
            .campaign_targeting_policy()
            .ok_or_else(|| self.training_error("hero classification is not admitted"))?;
        if self
            .campaign_mode
            .as_ref()
            .is_none_or(|mode| mode.image_sha256 != classifier.image_sha256)
        {
            return Err(self.training_error("guard-post classification image differs"));
        }
        let first = record.definition.rawcode[0];
        if (classifier.hero_first..=classifier.hero_last).contains(&first) {
            return Ok(false);
        }
        if super::unit_types::structure(&record.definition).map_err(|e| self.training_error(e))?
            || super::unit_types::class(&record.definition, "UNIT_TYPE_PEON")
                .map_err(|e| self.training_error(e))?
        {
            return Ok(false);
        }
        if record.cached || record.summoned {
            return Err(self.training_error("guard-post creation state is unsupported"));
        }
        Ok(true)
    }

    fn post_occupied(&self, post: &GuardPost, owner: HandleId) -> bool {
        post.unit
            .and_then(|id| self.executed_unit(id).ok())
            .is_some_and(|r| r.owner == owner && live(r))
    }

    pub(super) fn release_campaign_guard_post(&mut self, unit: HandleId) {
        let Some(policy) = &self.campaign_mode else {
            return;
        };
        let count = policy.guard_posts.released_count;
        let Some(player) = self
            .executed_unit(unit)
            .ok()
            .and_then(|r| self.player_settings(r.owner).ok())
            .map(|p| p.number)
        else {
            return;
        };
        if let Some(posts) = self.campaign_ai.guard_posts_mut(player) {
            for post in posts.iter_mut().filter(|p| p.unit == Some(unit)) {
                post.unit = None;
                post.assignments = count;
            }
        }
    }

    pub(super) fn prepare_guard_binding(
        &mut self,
        record: &ExecutedUnit,
        placement: &mut Option<crate::unit_placement::Prepared>,
    ) -> Result<Option<GuardBinding>, Error> {
        let player = self.player_settings(record.owner)?.number;
        let Some(posts) = self.campaign_ai.guard_posts(player) else {
            return Ok(None);
        };
        let Some(policy) = &self.campaign_mode else {
            return Ok(None);
        };
        let Some((index, post)) = posts
            .iter()
            .enumerate()
            .find(|(_, p)| {
                p.kind.to_be_bytes() == record.definition.rawcode
                    && !self.post_occupied(p, record.owner)
            })
            .map(|(index, post)| (index, *post))
        else {
            return Ok(None);
        };
        let limit = self
            .campaign_ai
            .replacement_count(player)
            .ok_or_else(|| self.training_error("replacement limit is unset"))?;
        if post.assignments > limit {
            return Ok(None);
        }
        if !self.ordinary_post_candidate(record)? {
            return Ok(None);
        }
        let id = self.inner.id(self.inner.arena.len(), NATIVE)?;
        let count = post
            .assignments
            .wrapping_add(policy.guard_posts.binding_increment);
        let prepared = placement
            .take()
            .ok_or_else(|| self.training_error("guard-post placement is missing"))?;
        let active = self
            .unit_placement
            .as_mut()
            .expect("prepared placement")
            .begin_creation_probe(id, prepared)
            .map_err(|e| self.training_error(e.to_string()))?;
        let route = self
            .prepare_record_steer(id, record, None, Some(post.home), 0., NATIVE)
            .and_then(|route| match route {
                Some(route) => Ok(route),
                None => self
                    .prepare_record_steer(id, record, None, None, 0., NATIVE)?
                    .ok_or_else(|| self.training_error("replacement stop preflight failed")),
            });
        *placement = Some(
            self.unit_placement
                .as_mut()
                .expect("prepared placement")
                .finish_creation_probe(id, active),
        );
        let route = route?;
        Ok(Some(GuardBinding {
            player,
            post: index,
            count,
            home: post.home,
            route,
        }))
    }

    pub(super) fn commit_guard_binding(
        &mut self,
        unit: HandleId,
        binding: GuardBinding,
    ) -> Result<(), Error> {
        self.commit_steer_unit(unit, binding.route, NATIVE)?;
        let post = &mut self
            .campaign_ai
            .guard_posts_mut(binding.player)
            .expect("active post")[binding.post];
        post.unit = Some(unit);
        post.assignments = binding.count;
        Ok(())
    }

    pub(crate) fn fill_ai_guard_posts(&mut self, player: i32) -> Result<(), Error> {
        self.require_active_campaign_ai_player(player, NATIVE)?;
        if self.campaign_mode.is_none() || self.guards.is_none() {
            return Err(self.training_error("guard-post rules are not admitted"));
        }
        let owner = self
            .player_identity(player)
            .ok_or_else(|| self.training_error("AI owner missing"))?;
        let count = self
            .campaign_ai
            .guard_posts(player)
            .expect("active player")
            .len();
        for index in 0..count {
            let post = self.campaign_ai.guard_posts(player).expect("active player")[index];
            if self.post_occupied(&post, owner)
                || self
                    .training
                    .jobs
                    .iter()
                    .any(|job| job.guard == Some((player, index)) && job.kind == post.kind)
            {
                continue;
            }
            let limit = self
                .campaign_ai
                .replacement_count(player)
                .ok_or_else(|| self.training_error("replacement limit is unset"))?;
            if post.assignments > limit {
                continue;
            }
            self.start_guard_training(player, owner, index, post.kind)?;
        }
        Ok(())
    }

    pub(crate) fn return_ai_guard_posts(&mut self, player: i32) -> Result<(), Error> {
        let name = "ReturnGuardPosts";
        self.require_active_campaign_ai_player(player, name)?;
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let policy = self
            .campaign_mode
            .as_ref()
            .ok_or_else(|| fail("guard-post rules are not admitted"))?;
        if self
            .order_registry
            .as_ref()
            .is_none_or(|r| r.binding().image_sha256 != policy.image_sha256)
            || !self.is_attack_order(policy.guard_posts.return_order)
        {
            return Err(fail("guard return order/source differs"));
        }
        let owner = self
            .player_identity(player)
            .ok_or_else(|| fail("AI owner missing"))?;
        let posts = self.campaign_ai.guard_posts(player).expect("active player");
        let mut routes = Vec::new();
        for post in posts {
            if !self.post_occupied(post, owner) {
                continue;
            }
            let unit = post.unit.expect("occupied post");
            let current_order = self
                .current_unit_order(unit)
                .map_err(|e| self.inner.error(name, e.failure))?;
            if current_order != 0 {
                continue;
            }
            let record = self.executed_unit(unit)?;
            if record.cached
                || record.summoned
                || record.admitted_order.is_some()
                || record.paused_since.is_some()
                || self.unit_has_effect_history(unit)
                || record.combat.is_none()
            {
                return Err(fail(&format!(
                    "guard return state is unsupported: AI player {player}, unit {unit:?}, rawcode {:?}, home {:?}, current_order {current_order}, cached {}, summoned {}, admitted_order {:?}, paused_since {:?}, shown {}, effect_history {}, combat_admitted {}",
                    record.definition.rawcode,
                    post.home,
                    record.cached,
                    record.summoned,
                    record.admitted_order,
                    record.paused_since,
                    record.shown,
                    self.unit_has_effect_history(unit),
                    record.combat.is_some(),
                )));
            }
            let position = record
                .position
                .ok_or_else(|| fail("guard position is not admitted"))?;
            if position == post.home {
                continue;
            }
            // shortcut: reuse canonical point routing; native arrival extent/polling remains to be qualified.
            if let Some(route) =
                self.prepare_steer_unit_in_range(unit, Some(post.home), 0., name)?
            {
                routes
                    .try_reserve(1)
                    .map_err(|_| fail("guard return capacity"))?;
                routes.push((unit, post.home, route));
            }
        }
        self.reserve_steer_commits(routes.len(), name)?;
        for (unit, home, route) in routes {
            self.commit_steer_unit(unit, route, name)?;
            self.clear_combat_target(unit);
            if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                record
                    .combat
                    .as_mut()
                    .expect("admitted combat")
                    .attack_destination = Some(home);
            }
        }
        Ok(())
    }

    /// Cancel one queue entry; public script order strings are not cancellation commands.
    pub fn cancel_unit_training(&mut self, producer: HandleId, slot: usize) -> Result<bool, Error> {
        let record = self.executed_unit(producer)?;
        if !live(record) {
            return Ok(false);
        }
        let Some(index) = self
            .training
            .jobs
            .iter()
            .enumerate()
            .filter(|(_, job)| job.producer == producer)
            .nth(slot)
            .map(|(index, _)| index)
        else {
            return Ok(false);
        };
        let job = self.training.jobs[index];
        if record.owner != job.owner {
            return Err(self.training_error("training owner changed"));
        }
        let paused = record.paused_since.is_some();
        let refund = [
            self.training_refund(job.gold)?,
            self.training_refund(job.lumber)?,
        ];
        let cancel = self.prepare_training_events(producer, "CANCEL")?;
        let mut next = None;
        let mut start = Vec::new();
        let released = if job.active { job.food } else { 0 };
        if job.active
            && !paused
            && let Some((next_index, mut head)) = self
                .training
                .jobs
                .iter()
                .enumerate()
                .skip(index + 1)
                .find(|(_, head)| head.producer == producer)
                .map(|(i, head)| (i, *head))
        {
            let used = self
                .player_resource_value(job.owner, "PLAYER_STATE_RESOURCE_FOOD_USED")?
                .ok_or_else(|| self.training_error("food not admitted"))?;
            let cap = self
                .player_resource_value(job.owner, "PLAYER_STATE_RESOURCE_FOOD_CAP")?
                .ok_or_else(|| self.training_error("food cap not admitted"))?;
            let after = used
                .checked_sub(released)
                .and_then(|n| n.checked_add(head.food))
                .ok_or_else(|| self.training_error("replacement food overflow"))?;
            if after <= cap {
                let now = self
                    .timer_game_time()
                    .filter(|n| n.is_finite() && *n >= 0.)
                    .ok_or_else(|| self.training_error("training clock not admitted"))?;
                head.active = true;
                head.through = now;
                head.finish = now + f64::from(head.duration);
                if !head.finish.is_finite() || head.finish <= now {
                    return Err(self.training_error("replacement deadline cannot advance"));
                }
                start = self.prepare_training_events(producer, "START")?;
                next = Some((next_index, head));
            }
        }
        let food = next
            .as_ref()
            .map_or(0, |(_, head)| head.food)
            .checked_sub(released)
            .ok_or_else(|| self.training_error("cancel food overflow"))?;
        let events = cancel
            .len()
            .checked_add(start.len())
            .ok_or_else(|| self.training_error("cancel event overflow"))?;
        let credit = self
            .prepare_training_change(job.owner, refund, food, events)?
            .ok_or_else(|| self.training_error("cancel resource transition refused"))?;
        if let Some((next_index, head)) = next {
            self.training.jobs[next_index] = head;
        }
        self.training.jobs.remove(index);
        self.commit_training_debit(credit);
        self.commit_training_events(cancel, producer, job.kind, None);
        if let Some((_, head)) = next {
            self.commit_training_events(start, producer, head.kind, None);
        }
        Ok(true)
    }

    pub(super) fn prepare_training_death(
        &mut self,
        producer: HandleId,
    ) -> Result<Option<TrainingDeath>, Error> {
        let current_owner = self.executed_unit(producer)?.owner;
        let mut owner = None;
        let mut refund = [0_i32; 2];
        let mut food = 0_i32;
        for job in self
            .training
            .jobs
            .iter()
            .filter(|job| job.producer == producer)
        {
            if current_owner != job.owner || owner.is_some_and(|owner| owner != job.owner) {
                return Err(self.training_error("queue owners differ"));
            }
            owner = Some(job.owner);
            for (sum, cost) in refund.iter_mut().zip([job.gold, job.lumber]) {
                *sum = sum
                    .checked_add(self.training_refund(cost)?)
                    .ok_or_else(|| self.training_error("death refund overflow"))?;
            }
            if job.active {
                food = food
                    .checked_add(job.food)
                    .ok_or_else(|| self.training_error("death food overflow"))?;
            }
        }
        let Some(owner) = owner else {
            return Ok(None);
        };
        let credit = self
            .prepare_training_change(owner, refund, -food, 0)?
            .ok_or_else(|| self.training_error("death refund refused"))?;
        Ok(Some(TrainingDeath { producer, credit }))
    }

    pub(super) fn commit_training_death(&mut self, death: TrainingDeath) {
        self.training
            .jobs
            .retain(|job| job.producer != death.producer);
        self.commit_training_debit(death.credit);
    }

    /// FIFO entries with a completion clock only for the active head.
    pub fn unit_training_queue(
        &self,
        producer: HandleId,
    ) -> Result<Vec<(i32, Option<f64>)>, Error> {
        self.executed_unit(producer)?;
        let count = self
            .training
            .jobs
            .iter()
            .filter(|job| job.producer == producer)
            .count();
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| self.training_error("queue observation capacity"))?;
        entries.extend(
            self.training
                .jobs
                .iter()
                .filter(|job| job.producer == producer)
                .map(|job| (job.kind, job.active.then_some(job.finish))),
        );
        Ok(entries)
    }

    pub(super) fn training_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != "IssueImmediateOrderById" {
            return None;
        }
        let [Value::Handle(producer), Value::Int(kind)] = args else {
            return None;
        };
        // Match the unit catalog's FourCC domain before asking synthetic catalogs to resolve it.
        if !kind.to_be_bytes().iter().all(u8::is_ascii_graphic) {
            return None;
        }
        let definition = self.unit_catalog().ok()?.resolve(kind.to_be_bytes()).ok()?;
        Some(
            (|| {
                Self::contract(
                    &self.inner,
                    name,
                    &["unit", "integer"],
                    "boolean",
                    false,
                    None,
                )?;
                if self
                    .inner
                    .program
                    .natives()
                    .get(name)
                    .expect("checked")
                    .source
                    != self.unit_declaration()?.0
                {
                    return Err(self.inner.error(name, Failure::Source));
                }
                let record = self.executed_unit(*producer)?;
                if !live(record) || record.construction.is_some() {
                    return Ok(Value::Bool(false));
                }
                let list = profile(&record.definition, "Trains", "unitList")
                    .map_err(|e| self.training_error(e))?;
                let mut allowed = false;
                for token in list.split(',').filter(|s| !s.trim().is_empty()) {
                    let token = token.trim();
                    if token.len() != 4 || !token.bytes().all(|b| b.is_ascii_graphic()) {
                        return Err(self.training_error("malformed Trains rawcode"));
                    }
                    allowed |= token.as_bytes() == kind.to_be_bytes();
                }
                if !allowed {
                    return Ok(Value::Bool(false));
                }
                if cached_without_rally(record)
                    || record.summoned
                    || record.admitted_order.is_some()
                    || self.unit_has_effect_history(*producer)
                {
                    return Err(self.training_error("producer state is unsupported"));
                }
                if record.paused_since.is_some() {
                    return Ok(Value::Bool(false));
                }
                let owner = record.owner;
                let slots = self.training_queue_slots()?;
                if self
                    .training
                    .jobs
                    .iter()
                    .filter(|j| j.producer == *producer)
                    .count()
                    >= slots
                {
                    return Ok(Value::Bool(false));
                }
                if super::unit_types::structure(&definition).map_err(|e| self.training_error(e))? {
                    return Err(self.training_error("construction is not a training order"));
                }
                self.enqueue_training(*producer, owner, None, *kind, definition)
                    .map(Value::Bool)
            })()
            .map_err(|e: Error| self.inner.error(name, e.failure)),
        )
    }

    fn start_guard_training(
        &mut self,
        player: i32,
        owner: HandleId,
        post: usize,
        kind: i32,
    ) -> Result<(), Error> {
        let requested = kind.to_be_bytes();
        let definition = self.unit_catalog()?.resolve(requested).map_err(|problem| {
            self.inner.error(
                NATIVE,
                Failure::UnitCatalog {
                    requested,
                    problem: Box::new(problem),
                },
            )
        })?;
        if definition.rawcode != requested
            || definition.kind != pcx_objects::editor::ObjectKind::Unit
        {
            return Err(self.training_error("training catalog identity differs"));
        }
        if super::unit_types::structure(&definition).map_err(|e| self.training_error(e))? {
            return Err(self.training_error("guard-post construction is not admitted"));
        }
        let selected = self.select_training_producer(owner, requested, true)?;
        let Some(producer) = selected else {
            return Ok(());
        };
        self.enqueue_training(producer, owner, Some((player, post)), kind, definition)
            .map(|_| ())
    }

    fn select_training_producer(
        &self,
        owner: HandleId,
        requested: [u8; 4],
        idle_only: bool,
    ) -> Result<Option<HandleId>, Error> {
        let slots = if idle_only {
            0
        } else {
            self.training_queue_slots()?
        };
        // shortcut: canonical arena order is the recreation tie-break, native producer preference remains unqualified.
        let mut selected = None;
        for (id, record) in self.executed_units() {
            if record.owner != owner
                || !live(record)
                || record.construction.is_some()
                || record.paused_since.is_some()
                || if idle_only {
                    self.training.jobs.iter().any(|j| j.producer == id)
                } else {
                    self.training
                        .jobs
                        .iter()
                        .filter(|j| j.producer == id)
                        .count()
                        >= slots
                }
            {
                continue;
            }
            let list = profile(&record.definition, "Trains", "unitList")
                .map_err(|e| self.training_error(e))?;
            let mut trains = false;
            for token in list.split(',').filter(|s| !s.trim().is_empty()) {
                let token = token.trim();
                if token.len() != 4 || !token.bytes().all(|b| b.is_ascii_graphic()) {
                    return Err(self.training_error("malformed Trains rawcode"));
                }
                trains |= token.as_bytes() == requested;
            }
            if trains {
                if cached_without_rally(record)
                    || record.summoned
                    || record.admitted_order.is_some()
                    || self.unit_has_effect_history(id)
                {
                    return Err(self.training_error("producer state is unsupported"));
                }
                selected = Some(id);
                break;
            }
        }
        Ok(selected)
    }

    pub(crate) fn set_ai_produce(
        &mut self,
        player: i32,
        quantity: i32,
        kind: i32,
        town: i32,
    ) -> Result<bool, Error> {
        let fail = |why: String| self.inner.error("SetProduce", Failure::UnitActivity(why));
        let maximum = self.admitted_ai_production_max()?;
        if !(1..=maximum).contains(&quantity) {
            return Ok(false);
        }
        if !self.campaign_ai.is_active(player) {
            return Err(fail(format!("AI player {player} is not active")));
        }
        let owner = self
            .player_identity(player)
            .ok_or_else(|| fail(format!("AI player {player} is not admitted")))?;
        if town != -1 {
            return Err(fail(format!(
                "town-specific production is not admitted: player={player} town={town}"
            )));
        }
        self.require_ordinary_training(kind)?;
        let requested = kind.to_be_bytes();
        let definition = self
            .unit_catalog()?
            .resolve(requested)
            .map_err(|problem| fail(format!("AI production catalog {requested:?}: {problem:?}")))?;
        if definition.rawcode != requested
            || definition.kind != pcx_objects::editor::ObjectKind::Unit
        {
            return Err(fail("AI production catalog identity differs".into()));
        }
        if super::unit_types::structure(&definition).map_err(fail)? {
            return Err(fail(format!(
                "AI construction is not admitted: player={player} kind={requested:?}"
            )));
        }
        for _ in 0..quantity {
            let Some(producer) = self.select_training_producer(owner, requested, false)? else {
                return Ok(false);
            };
            if !self.enqueue_training(producer, owner, None, kind, definition.clone())? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn enqueue_training(
        &mut self,
        producer: HandleId,
        owner: HandleId,
        guard: Option<(i32, usize)>,
        kind: i32,
        definition: Definition,
    ) -> Result<bool, Error> {
        self.require_ordinary_training(kind)?;
        if !self.training_prerequisites_met(&definition, owner)? {
            return Ok(false);
        }
        if !self.training_limit_allows(owner, kind)? {
            return Ok(false);
        }
        let gold = self.ai_unit_amount(owner, kind, "goldcost")?;
        let lumber = self.ai_unit_amount(owner, kind, "lumbercost")?;
        let duration = self.ai_unit_amount(owner, kind, "bldtm")?;
        let food = self
            .unit_food_amount(&definition, "fused")
            .map_err(|e| self.training_error(e))?;
        if duration <= 0 {
            return Err(self.training_error("training duration must be positive"));
        }
        let now = self
            .timer_game_time()
            .filter(|n| n.is_finite() && *n >= 0.)
            .ok_or_else(|| self.training_error("training clock is not admitted"))?;
        let completion = now + f64::from(duration);
        if !completion.is_finite() || completion <= now {
            return Err(self.training_error("training deadline cannot advance"));
        }
        let active = !self
            .training
            .jobs
            .iter()
            .any(|job| job.producer == producer);
        let budget = if guard.is_some() {
            self.guards.as_ref().expect("guard caller").capacity()
        } else {
            self.inner.max_handles
        };
        if self.training.jobs.len() >= budget {
            return Err(self.training_error("training job budget exhausted"));
        }
        self.training
            .jobs
            .try_reserve(1)
            .map_err(|_| self.training_error("training job capacity"))?;
        let events = if active {
            self.prepare_training_events(producer, "START")?
        } else {
            Vec::new()
        };
        let Some(debit) = self.prepare_training_debit(
            owner,
            gold,
            lumber,
            if active { food } else { 0 },
            events.len(),
        )?
        else {
            return Ok(false);
        };
        // shortcut: catalog duration uses the session clock; qualify native production polling before sub-tick parity claims.
        self.training.jobs.push(Training {
            producer,
            owner,
            guard,
            active,
            duration,
            gold,
            lumber,
            kind,
            food,
            finish: completion,
            through: now,
            pause: None,
        });
        self.commit_training_debit(debit);
        self.commit_training_events(events, producer, kind, None);
        Ok(true)
    }

    pub(super) fn pending_training_food(&self, owner: HandleId) -> Result<i32, Error> {
        self.training
            .jobs
            .iter()
            .filter(|j| j.owner == owner && j.active)
            .try_fold(0i32, |total, job| {
                total
                    .checked_add(job.food)
                    .ok_or_else(|| self.training_error("pending food overflow"))
            })
    }

    pub(super) fn pending_training_count(&self, owner: HandleId, kind: i32) -> Result<i32, Error> {
        i32::try_from(
            self.training
                .jobs
                .iter()
                .filter(|j| j.owner == owner && j.kind == kind)
                .count(),
        )
        .map_err(|_| self.training_error("pending count overflow"))
    }

    pub(super) fn advance_training(&mut self, now: f64, work: &mut usize) -> Result<(), Error> {
        let mut index = 0;
        while index < self.training.jobs.len() {
            *work = work
                .checked_sub(1)
                .ok_or_else(|| self.training_error("training work budget"))?;
            let mut job = self.training.jobs[index];
            if !now.is_finite() || now < job.through {
                return Err(self.training_error("training clock moved backwards"));
            }
            let producer = self
                .executed_unit(job.producer)
                .map_err(|e| self.inner.error(NATIVE, e.failure))?;
            if producer.owner != job.owner || !live(producer) {
                return Err(self.training_error(
                    "producer removal or transfer during training is unsupported",
                ));
            }
            if !job.active {
                if self.training.jobs[..index]
                    .iter()
                    .any(|other| other.producer == job.producer)
                    || producer.paused_since.is_some()
                {
                    index += 1;
                    continue;
                }
                let events = self.prepare_training_events(job.producer, "START")?;
                let Some(debit) =
                    self.prepare_training_debit(job.owner, 0, 0, job.food, events.len())?
                else {
                    index += 1;
                    continue;
                };
                job.active = true;
                job.finish = now + f64::from(job.duration);
                if !job.finish.is_finite() || job.finish <= now {
                    return Err(self.training_error("training deadline cannot advance"));
                }
                job.through = now;
                self.training.jobs[index] = job;
                self.commit_training_debit(debit);
                self.commit_training_events(events, job.producer, job.kind, None);
                index += 1;
                continue;
            }
            if let Some(since) = producer.paused_since {
                let (at, base) = *job
                    .pause
                    .get_or_insert((since.max(job.through), job.finish));
                job.finish = base + (now - at);
                if !job.finish.is_finite() {
                    return Err(self.training_error("paused training deadline overflow"));
                }
            } else {
                job.pause = None;
            }
            job.through = now;
            if producer.paused_since.is_some() || job.finish > now {
                self.training.jobs[index] = job;
                index += 1;
                continue;
            }
            let [x, y] = producer
                .position
                .ok_or_else(|| self.training_error("producer position is not admitted"))?;
            let face = producer.face;
            let events = self.prepare_training_events(job.producer, "FINISH")?;
            let prepared = self.prepare_unit_creation(
                &[
                    Value::Handle(job.owner),
                    Value::Int(job.kind),
                    Value::Real(x),
                    Value::Real(y),
                    Value::Real(face),
                ],
                None,
                super::unit_creation::Birth::Ordinary,
            )?;
            let rally = self
                .executed_unit(job.producer)?
                .activity
                .as_ref()
                .and_then(|activity| activity.rally);
            let unit = self.commit_unit_creation(prepared)?;
            if let Some(goal) = rally
                && let Some(route) =
                    self.prepare_steer_unit_in_range(unit, Some(goal), 0.0, "training")?
            {
                // Same admission as a movement point order: the guard post must not
                // treat this route as an idle return to its own home.
                self.commit_steer_unit(unit, route, "training")?;
                self.unit_ordered(unit);
                self.set_explicit_guard_move(unit, true);
                self.clear_combat_target(unit);
            }
            self.training.jobs.remove(index);
            self.commit_training_events(events, job.producer, job.kind, Some(unit));
        }
        Ok(())
    }

    pub(crate) fn next_training_completion(&self, until: f64) -> Option<f64> {
        self.training
            .jobs
            .iter()
            .filter(|job| job.active)
            .filter_map(|job| {
                let producer = self.executed_unit(job.producer).ok()?;
                if producer.paused_since.is_some() {
                    return None;
                }
                let at = job.finish.max(self.timer_game_time()?);
                (at.is_finite() && at <= until).then_some(at)
            })
            .min_by(f64::total_cmp)
    }

    fn prepare_training_events(
        &mut self,
        producer: HandleId,
        phase: &str,
    ) -> Result<Vec<TrainingEvent>, Error> {
        let source = self.unit_declaration()?.0;
        for (name, result) in [
            ("GetTrainedUnit", "unit"),
            ("GetTrainedUnitType", "integer"),
        ] {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, &[], result, true, None)?;
                if native.source != source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        let unit_name = format!("EVENT_UNIT_TRAIN_{phase}");
        let player_name = format!("EVENT_PLAYER_UNIT_TRAIN_{phase}");
        let unit_event = self.source_constant_enumerand(&unit_name, "unitevent")?;
        let player_event = self.source_constant_enumerand(&player_name, "playerunitevent")?;
        let owner = self.executed_unit(producer)?.owner;
        let mut events = Vec::new();
        for entry in &self.inner.arena {
            let row = match entry {
                Entry::RegisteredUnitEvent {
                    trigger,
                    unit,
                    unitevent,
                    enumerand,
                } if *unit == producer && *enumerand == unit_event => {
                    Some((*trigger, None, *unitevent, None))
                }
                Entry::RegisteredPlayerUnitEvent(r)
                    if r.player == owner && r.enumerand == player_event =>
                {
                    Some((r.trigger, r.filter, r.playerunitevent, Some(owner)))
                }
                _ => None,
            };
            if let Some((trigger, filter, event, player)) = row {
                let filter = self.resolve_event_filter(filter)?;
                events
                    .try_reserve(1)
                    .map_err(|_| self.training_error("training event capacity"))?;
                events.push(TrainingEvent {
                    trigger,
                    filter,
                    event,
                    player,
                });
            }
        }
        self.reserve_event_delivery(events.len())?;
        Ok(events)
    }

    fn commit_training_events(
        &mut self,
        events: Vec<TrainingEvent>,
        producer: HandleId,
        kind: i32,
        unit: Option<HandleId>,
    ) {
        for event in events {
            self.push_reserved_event(
                event.trigger,
                event.filter,
                EventSubject {
                    unit: Some(producer),
                    player: event.player,
                    trained: unit,
                    trained_type: Some(kind),
                    ..Default::default()
                },
                Some(event.event),
            );
        }
    }
}

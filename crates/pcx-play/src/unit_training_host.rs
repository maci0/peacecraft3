//! Shared producer queues and guard replacement through canonical unit creation.
use super::{Entry, Error, EventSubject, ExecutedUnit, Failure, ObjectHost};
use crate::training_rules::RefundKind;
use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
use pcx_script::{HandleId, Value, value::FunctionId};
use pcx_sim::world::{Definition, FieldProblem, FieldValue};

const NATIVE: &str = "FillGuardPosts";

/// A listed unit and its source identity and position.
type ListingUnit = (HandleId, [u8; 4], Option<[f32; 2]>);

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
    /// Unit type, or the upgrade id of a research job.
    kind: i32,
    /// The level a research job brings its owner to; `None` trains `kind`.
    research: Option<i32>,
    /// `kind` replaces this building. Not a trained unit and not a research level.
    structure_upgrade: bool,
    food: i32,
    finish: f64,
    through: f64,
    pause: Option<(f64, f64)>,
}

impl Training {
    fn refund_kind(&self) -> RefundKind {
        if self.structure_upgrade {
            RefundKind::StructureUpgrade
        } else if self.research.is_some() {
            RefundKind::Research
        } else {
            RefundKind::Training
        }
    }
}

#[derive(Default)]
pub(super) struct State {
    jobs: Vec<Training>,
}

pub(super) struct TrainingDeath {
    producer: HandleId,
    credit: Option<super::players::TrainingDebit>,
    upgrade: Option<UpgradeReplacement>,
}

struct UpgradeReplacement {
    form: super::unit_morph::Form,
    abilities: Option<super::UnitAbilities>,
    vitals: super::unit_vitals::Vitals,
    decay: Option<crate::unit_decay::State>,
    placement: crate::unit_placement::Prepared,
    revision: u64,
    radius: Result<f32, &'static str>,
    death_animation: Option<bool>,
}

impl TrainingDeath {
    pub(super) fn upgraded_decay(&self) -> Option<Option<crate::unit_decay::State>> {
        self.upgrade.as_ref().map(|upgrade| upgrade.decay.clone())
    }

    pub(super) fn upgraded_death_animation(&self) -> Option<bool> {
        self.upgrade
            .as_ref()
            .and_then(|upgrade| upgrade.death_animation)
    }
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

/// Why a training button is drawn disabled. Presentation only: the enqueue
/// path re-checks every gate and refuses there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainUnavailable {
    /// The producer is paused; the host refuses new paid orders there.
    Paused,
    /// The unit is a hero: ordinary training and token transitions are refused.
    Hero,
    /// The producer's queue already holds the source bound.
    QueueFull,
    /// The unit's `Requires` list is not met by the owner.
    Requirements,
    /// The owner's tech max for the unit is reached.
    Limit,
    /// The owner's gold or lumber bank is short.
    Resources,
    /// The head would reserve food past the cap. Pending entries reserve no
    /// food, so this only applies when the queue is empty.
    Food,
}

/// One trainable unit of a producer with its authored cost and readiness.
/// Costs resolve the same unit catalog and map overrides the enqueue path
/// uses; `refund_gold`/`refund_lumber` are the per-entry cancellation refunds
/// through the source numeric policy, truncated per entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrainOption {
    pub kind: i32,
    pub gold: i32,
    pub lumber: i32,
    pub food: i32,
    pub duration: i32,
    pub refund_gold: i32,
    pub refund_lumber: i32,
    pub unavailable: Option<TrainUnavailable>,
}

/// One queued training entry for presentation. `finish`/`through` are the
/// live head clocks only when `active` is set; pending entries carry `None`,
/// matching [`ObjectHost::unit_training_queue`]. Refunds are per-entry
/// conversions, not a share of a summed cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrainingQueueEntry {
    pub kind: i32,
    pub active: bool,
    pub gold: i32,
    pub lumber: i32,
    pub refund_gold: i32,
    pub refund_lumber: i32,
    pub food: i32,
    pub duration: i32,
    pub finish: Option<f64>,
    pub through: Option<f64>,
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
        let limit = self.replacement_limit(player, policy.default_replacement);
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

    /// The script's replacement count, else the context's initial one.
    fn replacement_limit(&self, player: i32, default: i32) -> i32 {
        self.campaign_ai
            .replacement_count(player)
            .unwrap_or(default)
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
            let default = self
                .campaign_mode
                .as_ref()
                .expect("checked above")
                .default_replacement;
            let limit = self.replacement_limit(player, default);
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
                || record.admitted_order.is_some_and(|(_, active)| active)
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

    /// Whether `producer` holds any queued training job (active or pending).
    pub(crate) fn producer_has_training(&self, producer: HandleId) -> bool {
        self.training
            .jobs
            .iter()
            .any(|job| job.producer == producer)
    }

    pub(super) fn producer_is_upgrading(&self, producer: HandleId) -> bool {
        self.training
            .jobs
            .iter()
            .any(|job| job.producer == producer && job.structure_upgrade)
    }

    pub(super) fn transfer_structure_upgrade(&mut self, producer: HandleId, owner: HandleId) {
        for job in &mut self.training.jobs {
            if job.producer == producer && job.structure_upgrade {
                job.owner = owner;
            }
        }
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
            self.training_refund(job.gold, job.refund_kind())?,
            self.training_refund(job.lumber, job.refund_kind())?,
        ];
        let cancel = self.prepare_job_events(&job, producer, "CANCEL")?;
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
                start = self.prepare_job_events(&head, producer, "START")?;
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
        let credit = if job.structure_upgrade {
            self.prepare_structure_upgrade_payment(job.owner, refund, true, events)?
        } else {
            self.prepare_training_change(job.owner, refund, food, events)?
        }
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
        self.prepare_training_loss(producer, true)
    }

    pub(super) fn prepare_training_removal(
        &mut self,
        producer: HandleId,
    ) -> Result<Option<TrainingDeath>, Error> {
        self.prepare_training_loss(producer, false)
    }

    fn prepare_training_loss(
        &mut self,
        producer: HandleId,
        dying: bool,
    ) -> Result<Option<TrainingDeath>, Error> {
        let current_owner = self.executed_unit(producer)?.owner;
        let mut owner = None;
        let mut refund = [0_i32; 2];
        let mut food = 0_i32;
        let mut upgrading = None;
        let mut ordinary = false;
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
            if job.structure_upgrade {
                upgrading = Some(job.kind);
                continue;
            }
            ordinary = true;
            for (sum, cost) in refund.iter_mut().zip([job.gold, job.lumber]) {
                *sum = sum
                    .checked_add(self.training_refund(cost, job.refund_kind())?)
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
        if upgrading.is_some() && ordinary {
            return Err(self.training_error("mixed upgrade and training queue is not qualified"));
        }
        let upgrade = if dying && let Some(kind) = upgrading {
            let mut upgrade = self.prepare_upgrade_replacement(producer, kind)?;
            let record = self.executed_unit(producer)?;
            let levels = |abilities: &Option<super::UnitAbilities>| {
                abilities.as_ref().map(|a| a.levels().collect::<Vec<_>>())
            };
            if !super::unit_types::structure(&record.definition)
                .map_err(|e| self.training_error(e))?
                || !super::unit_types::structure(&upgrade.form.definition)
                    .map_err(|e| self.training_error(e))?
                || super::unit_types::flying(&record.definition)
                    != super::unit_types::flying(&upgrade.form.definition)
                || record.abilities.as_ref().is_some_and(|a| a.hero.is_some())
                || upgrade.abilities.as_ref().is_some_and(|a| a.hero.is_some())
                || levels(&record.abilities) != levels(&upgrade.abilities)
                || super::combat::column(&record.definition, SlkId::UnitBalance, "level")
                    .map_err(|e| self.training_error(e))?
                    != super::combat::column(&upgrade.form.definition, SlkId::UnitBalance, "level")
                        .map_err(|e| self.training_error(e))?
            {
                return Err(self.training_error(
                    "upgrade death changes an unqualified award, ability or movement class",
                ));
            }
            let now = self
                .timer_game_time()
                .ok_or_else(|| self.training_error("upgrade death clock is not admitted"))?;
            if let Some(decay) = &mut upgrade.decay {
                decay.begin(now).map_err(|e| self.training_error(e))?;
            }
            let mut exploded = false;
            if let Some(animation) = &mut upgrade.form.animation {
                exploded = self.unit_exploded(producer) == Some(true)
                    && animation.has_sequence("death explode");
                let sequence = if exploded { "death explode" } else { "death" };
                let selected = if !exploded && let Some(decay) = &upgrade.decay {
                    animation.select_for_duration(sequence, now, decay.death)
                } else {
                    animation.select_engine(sequence, now, None)
                };
                selected.map_err(|e| self.training_error(e))?;
            }
            upgrade.death_animation = Some(exploded);
            upgrade.placement.deactivate();
            Some(upgrade)
        } else {
            None
        };
        let credit = if ordinary {
            Some(
                self.prepare_training_change(owner, refund, -food, 0)?
                    .ok_or_else(|| self.training_error("death refund refused"))?,
            )
        } else {
            None
        };
        Ok(Some(TrainingDeath {
            producer,
            credit,
            upgrade,
        }))
    }

    pub(super) fn commit_training_death(&mut self, death: TrainingDeath) {
        self.training
            .jobs
            .retain(|job| job.producer != death.producer);
        if let Some(upgrade) = death.upgrade {
            self.commit_upgrade_replacement(death.producer, upgrade);
        }
        if let Some(credit) = death.credit {
            self.commit_training_debit(credit);
        }
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

    /// The source queue bound, for `used/max` presentation. Refuses when the
    /// bound is not admitted rather than presenting an invented limit.
    pub fn training_queue_capacity(&self) -> Result<usize, Error> {
        self.training_queue_slots()
    }

    /// Authored trainable units of `producer` with resolved costs and
    /// readiness. Order follows the producer's `Trains` list with repeats
    /// removed, matching the command card. A removed producer errors so the
    /// presentation hides instead of showing a stale queue; a dead producer
    /// yields no options. Explicit owner transfer is not resolved here: the
    /// caller compares the producer's live owner with its selection.
    pub fn unit_train_options(&self, producer: HandleId) -> Result<Vec<TrainOption>, Error> {
        let record = self.executed_unit(producer)?;
        if self.producer_is_upgrading(producer) {
            return Err(self.training_error("training during a structure upgrade is not qualified"));
        }
        let owner = record.owner;
        if !live(record) {
            return Ok(Vec::new());
        }
        let paused = record.paused_since.is_some();
        let slots = self.training_queue_slots()?;
        let queued = self
            .training
            .jobs
            .iter()
            .filter(|job| job.producer == producer)
            .count();
        let list = profile(&record.definition, "Trains", "unitList")
            .map_err(|e| self.training_error(e))?;
        let mut kinds = Vec::new();
        for token in list.split(',').filter(|s| !s.trim().is_empty()) {
            if matches!(token.trim(), "_" | "-") {
                continue;
            }
            let code: [u8; 4] = token
                .trim()
                .as_bytes()
                .try_into()
                .map_err(|_| self.training_error("malformed Trains rawcode"))?;
            if !code.iter().all(u8::is_ascii_graphic) {
                return Err(self.training_error("malformed Trains rawcode"));
            }
            let kind = i32::from_be_bytes(code);
            if kinds.contains(&kind) {
                continue;
            }
            kinds.push(kind);
        }
        let mut options = Vec::new();
        options
            .try_reserve_exact(kinds.len())
            .map_err(|_| self.training_error("training option capacity"))?;
        for kind in kinds {
            let hero = self.require_ordinary_training(kind).is_err();
            let gold = self.ai_unit_amount(owner, kind, "goldcost")?;
            let lumber = self.ai_unit_amount(owner, kind, "lumbercost")?;
            // Hero build times need revival state the host does not admit;
            // the button stays disabled on the hero gate, so no time is shown.
            let duration = if hero {
                0
            } else {
                self.ai_unit_amount(owner, kind, "bldtm")?
            };
            let definition =
                self.unit_catalog()?
                    .resolve(kind.to_be_bytes())
                    .map_err(|problem| {
                        self.training_error(format!("training catalog {kind:?}: {problem:?}"))
                    })?;
            let food = self
                .unit_food_amount(&definition, "fused")
                .map_err(|e| self.training_error(e))?;
            let prerequisites = self.training_prerequisites_met(&definition, owner)?;
            let limit = self.training_limit_allows(owner, kind)?;
            let bank_gold = self
                .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")?
                .ok_or_else(|| self.training_error("training gold not admitted"))?;
            let bank_lumber = self
                .player_resource_value(owner, "PLAYER_STATE_RESOURCE_LUMBER")?
                .ok_or_else(|| self.training_error("training lumber not admitted"))?;
            // A non-empty queue takes the entry pending with no food
            // reservation, so the cap only gates the activating head.
            let food_blocked = if queued == 0 && food > 0 {
                let used = self
                    .player_resource_value(owner, "PLAYER_STATE_RESOURCE_FOOD_USED")?
                    .ok_or_else(|| self.training_error("training food not admitted"))?;
                let cap = self
                    .player_resource_value(owner, "PLAYER_STATE_RESOURCE_FOOD_CAP")?
                    .ok_or_else(|| self.training_error("food capacity not admitted"))?;
                used.checked_add(food).is_none_or(|after| after > cap)
            } else {
                false
            };
            let unavailable = if paused {
                Some(TrainUnavailable::Paused)
            } else if hero {
                Some(TrainUnavailable::Hero)
            } else if queued >= slots {
                Some(TrainUnavailable::QueueFull)
            } else if !prerequisites {
                Some(TrainUnavailable::Requirements)
            } else if !limit {
                Some(TrainUnavailable::Limit)
            } else if gold > bank_gold || lumber > bank_lumber {
                Some(TrainUnavailable::Resources)
            } else if food_blocked {
                Some(TrainUnavailable::Food)
            } else {
                None
            };
            options.push(TrainOption {
                kind,
                gold,
                lumber,
                food,
                duration,
                refund_gold: self.training_refund(gold, RefundKind::Training)?,
                refund_lumber: self.training_refund(lumber, RefundKind::Training)?,
                unavailable,
            });
        }
        Ok(options)
    }

    /// The producer's queue with per-entry costs and source-derived refunds.
    /// A removed producer errors so the presentation hides; an owner change
    /// against the jobs is reported rather than presenting another owner's
    /// queue as this producer's.
    pub fn unit_training_details(
        &self,
        producer: HandleId,
    ) -> Result<Vec<TrainingQueueEntry>, Error> {
        let record = self.executed_unit(producer)?;
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
        for job in self
            .training
            .jobs
            .iter()
            .filter(|job| job.producer == producer)
        {
            if record.owner != job.owner {
                return Err(self.training_error("training owner changed"));
            }
            entries.push(TrainingQueueEntry {
                kind: job.kind,
                active: job.active,
                gold: job.gold,
                lumber: job.lumber,
                refund_gold: self.training_refund(job.gold, job.refund_kind())?,
                refund_lumber: self.training_refund(job.lumber, job.refund_kind())?,
                food: job.food,
                duration: job.duration,
                finish: job.active.then_some(job.finish),
                through: job.active.then_some(job.through),
            });
        }
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

    /// Live units of `owner` that list `kind` in the `key` profile list, in arena order.
    fn ai_units_listing(
        &self,
        owner: HandleId,
        kind: [u8; 4],
        key: &str,
    ) -> Result<Vec<ListingUnit>, Error> {
        let mut listed = Vec::new();
        for (unit, record) in self.executed_units() {
            if record.owner != owner || !live(record) {
                continue;
            }
            let lists_builds = record.definition.fields.values().any(|field| {
                field.metadata.as_ref().is_some_and(|metadata| {
                    matches!(&metadata.source, FieldSource::Profile { key: k, .. }
                        if k.eq_ignore_ascii_case(key))
                })
            });
            if !lists_builds {
                continue;
            }
            let list =
                profile(&record.definition, key, "unitList").map_err(|e| self.training_error(e))?;
            let mut found = false;
            for token in list.split(',').map(str::trim) {
                if matches!(token, "" | "_" | "-") {
                    continue;
                }
                if token.len() != 4 || !token.bytes().all(|b| b.is_ascii_graphic()) {
                    return Err(self.training_error(format!("malformed {key} rawcode")));
                }
                found |= token.as_bytes() == kind;
            }
            if found {
                if listed.len() >= self.inner.max_handles {
                    return Err(self.training_error("builder list exhausted"));
                }
                listed
                    .try_reserve(1)
                    .map_err(|_| self.training_error("builder list capacity"))?;
                listed.push((unit, record.definition.rawcode, record.position));
            }
        }
        Ok(listed)
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
            // `0xac2b00` walks the player's units and keeps going when one fails the
            // candidate checks. An admitted construction order stands in for that
            // rejection: the orange probe keeps the first acolyte on the necropolis
            // and gives the next structures to other acolytes. `unit+0x3a0` and the
            // component bits `0x30000000` are still unmapped. With no free builder
            // and no site, the request answers false and orders nothing. The upgrade
            // fallback still runs only when no live unit lists the kind in `Builds`.
            let builders = self.ai_units_listing(owner, requested, "Builds")?;
            if !builders.is_empty() {
                for (worker, _rawcode, position) in builders {
                    if self.unit_construction_order(worker)?.is_some() {
                        continue;
                    }
                    #[cfg(feature = "test-fixtures")]
                    if std::env::var_os("PCX_AI_PLACEMENT_TRACE").is_some() {
                        eprintln!(
                            "AI build search: time={:?} player={player} kind={requested:?} worker={worker:?} position={position:?}",
                            self.timer_game_time()
                        );
                    }
                    let _ = position;
                    let (site, _dequeues) = self
                        .ai_structure_site(player, owner, worker, &definition)
                        .map_err(|why| {
                            self.inner.error(
                                "SetProduce",
                                Failure::UnitActivity(format!(
                                    "AI structure {} by {worker:?}: {why}",
                                    String::from_utf8_lossy(&requested)
                                )),
                            )
                        })?;
                    match site {
                        Some(super::ai_construction::AiBuildTarget::Point(site)) => {
                            let accepted = self.request_unit_construction(worker, kind, site)?;
                            #[cfg(feature = "test-fixtures")]
                            if std::env::var_os("PCX_AI_PLACEMENT_TRACE").is_some() {
                                eprintln!(
                                    "AI build order: time={:?} player={player} kind={requested:?} worker={worker:?} site={site:?} dequeues={_dequeues} accepted={accepted}",
                                    self.timer_game_time()
                                );
                            }
                            return Ok(accepted);
                        }
                        Some(super::ai_construction::AiBuildTarget::Mine {
                            mine,
                            position,
                            candidate,
                        }) => {
                            let accepted =
                                self.request_linked_mine_construction(worker, kind, mine)?;
                            #[cfg(feature = "test-fixtures")]
                            if std::env::var_os("PCX_AI_PLACEMENT_TRACE").is_some() {
                                eprintln!(
                                    "AI mine order: time={:?} player={player} kind={requested:?} worker={worker:?} mine={mine:?} position={position:?} candidate={candidate:?} dequeues={_dequeues} accepted={accepted}",
                                    self.timer_game_time()
                                );
                            }
                            let _ = (position, candidate);
                            return Ok(accepted);
                        }
                        None => continue,
                    }
                }
                return Ok(false);
            }
            // `0xac3070` issues the upgrade when no builder was found and the
            // request is not `ugol`. Idle is an empty queue and no construction
            // site (the counters at `+0x3c` and `+0x40` are not decoded). The
            // target's `Requires` stands in for check slot `0x328`. Source cost
            // mode prices the job; its source upgrade rate controls refunds.
            // Completion retains the life percentage in the target's base pool;
            // researched bonuses are not rebased onto that base.
            if requested == *b"ugol" || !self.training_prerequisites_met(&definition, owner)? {
                return Ok(false);
            }
            let gold = self.ai_unit_amount(owner, kind, "goldcost")?;
            let lumber = self.ai_unit_amount(owner, kind, "lumbercost")?;
            let duration = self.ai_unit_amount(owner, kind, "bldtm")?;
            if duration <= 0 {
                return Err(fail("structure upgrade duration must be positive".into()));
            }
            for _ in 0..quantity {
                let candidates = self.ai_units_listing(owner, requested, "Upgrade")?;
                let mut chosen = None;
                for (worker, _, _) in candidates {
                    let record = self.executed_unit(worker)?;
                    if record.construction.is_some()
                        || record.construction_request.is_some()
                        || !record.shown
                        || record.paused_since.is_some()
                        || self.training.jobs.iter().any(|job| job.producer == worker)
                    {
                        continue;
                    }
                    chosen = Some(worker);
                    break;
                }
                let Some(producer) = chosen else {
                    return Ok(false);
                };
                let [gold, lumber] = self.structure_upgrade_costs(
                    &self.executed_unit(producer)?.definition,
                    [gold, lumber],
                )?;
                if self.training.jobs.len() >= self.inner.max_handles {
                    return Err(self.inner.error(
                        "SetProduce",
                        Failure::UnitActivity("structure upgrade job budget exhausted".into()),
                    ));
                }
                if self.training.jobs.try_reserve(1).is_err() {
                    return Err(self.inner.error(
                        "SetProduce",
                        Failure::UnitActivity("structure upgrade job capacity".into()),
                    ));
                }
                let now = self.timer_game_time().filter(|n| n.is_finite() && *n >= 0.);
                let Some(now) = now else {
                    return Err(self.inner.error(
                        "SetProduce",
                        Failure::UnitActivity("structure upgrade clock is not admitted".into()),
                    ));
                };
                let finish = now + f64::from(duration);
                if !finish.is_finite() || finish <= now {
                    return Err(self.inner.error(
                        "SetProduce",
                        Failure::UnitActivity("structure upgrade deadline cannot advance".into()),
                    ));
                }
                self.prepare_upgrade_vitals(producer, &definition)?;
                let Some(debit) =
                    self.prepare_structure_upgrade_payment(owner, [gold, lumber], false, 0)?
                else {
                    return Ok(false);
                };
                self.training.jobs.push(Training {
                    producer,
                    owner,
                    guard: None,
                    active: true,
                    duration,
                    gold,
                    lumber,
                    kind,
                    research: None,
                    structure_upgrade: true,
                    food: 0,
                    finish,
                    through: now,
                    pause: None,
                });
                self.commit_training_debit(debit);
            }
            return Ok(true);
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
        if self.producer_is_upgrading(producer) {
            return Err(self.training_error("training during a structure upgrade is not qualified"));
        }
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
            research: None,
            structure_upgrade: false,
            food,
            finish: completion,
            through: now,
            pause: None,
        });
        self.commit_training_debit(debit);
        self.commit_training_events(events, producer, kind, None);
        Ok(true)
    }

    /// Whether `owner` has a research of `upgrade` queued or running.
    pub(crate) fn has_pending_research(&self, owner: HandleId, upgrade: i32) -> bool {
        self.pending_research(owner, upgrade)
    }

    fn pending_research(&self, owner: HandleId, upgrade: i32) -> bool {
        self.training
            .jobs
            .iter()
            .any(|job| job.owner == owner && job.research.is_some() && job.kind == upgrade)
    }

    fn research_error(&self, detail: impl Into<String>) -> Error {
        self.inner
            .error("SetUpgrade", Failure::UnitActivity(detail.into()))
    }

    /// The AI's view of an upgrade level: the researched level, plus one while a research of it
    /// is queued or running (exe 0xab5570 adds one when the player's in-progress count for the
    /// upgrade, flag 2 of 0x513620, is positive).
    pub(crate) fn ai_upgrade_level(&self, owner: HandleId, upgrade: i32) -> Result<i32, Error> {
        let Some(level) = self.stored_upgrade_level(owner, upgrade)? else {
            return Err(self.research_error("upgrade levels are not admitted for this upgrade"));
        };
        Ok(level + i32::from(self.pending_research(owner, upgrade)))
    }

    /// `GetUpgradeGoldCost` and `GetUpgradeWoodCost`: the cost of the level after the AI's
    /// level (exe 0xab5550 and 0xab5620 call 0x6df7e0 and 0x6e0a30 with that level).
    pub(crate) fn ai_upgrade_cost(
        &self,
        owner: HandleId,
        upgrade: i32,
        lumber: bool,
    ) -> Result<i32, Error> {
        let level = self
            .ai_upgrade_level(owner, upgrade)?
            .checked_add(1)
            .ok_or_else(|| self.research_error("upgrade level overflow"))?;
        let research = self.research_rules(upgrade)?;
        let prices = research.prices();
        let cost = if lumber {
            prices.lumber(level)
        } else {
            prices.gold(level)
        };
        cost.ok_or_else(|| self.research_error("upgrade cost overflow"))
    }

    fn research_rules(&self, upgrade: i32) -> Result<&crate::upgrade_research::Research, Error> {
        self.active_upgrade_rules()
            .ok_or_else(|| self.research_error("the upgrade model is not admitted"))?
            .research(upgrade)
            .map_err(|e| self.research_error(e))
    }

    /// `SetUpgrade`: the first idle building of the AI player that researches `upgrade` takes
    /// the order when the player can afford it, as the executable's loop at 0xac3ac0 does over
    /// the player's units. It orders nothing when no building qualifies. The native itself
    /// returns false either way (0xab7680 zeroes the result after the call).
    ///
    /// Established: the cost levels, the loop shape, the order of the checks (can research,
    /// idle, requirements and cost). Recreation readings: arena order for the unit order, "idle"
    /// as an empty queue (the executable tests two counters at +0x3c and +0x40 of the queue
    /// ability, not decoded), the `Researches` list for "can research", `Requires` keys for the
    /// requirements, and the cost paid when the order is accepted, refunded on cancel and on
    /// the producer's death like a training cost. Refused: a second research of an upgrade that
    /// is already running, a level beyond `maxlevel`, and a producer under pause or transfer.
    pub(crate) fn set_ai_upgrade(&mut self, player: i32, upgrade: i32) -> Result<(), Error> {
        if !self.campaign_ai.is_active(player) {
            return Err(self.research_error(format!("AI player {player} is not active")));
        }
        let owner = self
            .player_identity(player)
            .ok_or_else(|| self.research_error(format!("AI player {player} is not admitted")))?;
        if self.pending_research(owner, upgrade) {
            return Err(self.research_error(
                "a second research of an upgrade already running is not established",
            ));
        }
        let level = self
            .ai_upgrade_level(owner, upgrade)?
            .checked_add(1)
            .ok_or_else(|| self.research_error("upgrade level overflow"))?;
        let (maximum, requires, seconds, gold, lumber) = {
            let rules = self
                .active_upgrade_rules()
                .ok_or_else(|| self.research_error("the upgrade model is not admitted"))?;
            let research = rules
                .research(upgrade)
                .map_err(|e| self.research_error(e))?;
            let prices = research.prices();
            let cost = |value: Option<i32>| {
                value.ok_or_else(|| self.research_error("upgrade price overflow"))
            };
            (
                rules.maximum(upgrade),
                research.requires(level).to_vec(),
                cost(prices.seconds(level))?,
                cost(prices.gold(level))?,
                cost(prices.lumber(level))?,
            )
        };
        if maximum.is_none_or(|maximum| level > maximum) {
            return Err(self.research_error(format!(
                "research of level {level} is beyond the upgrade's levels"
            )));
        }
        if seconds <= 0 {
            return Err(self.research_error("research duration must be positive"));
        }
        for code in requires {
            if !self.technology_requirement_met(owner, i32::from_be_bytes(code), None)? {
                return Ok(());
            }
        }
        let mut chosen = None;
        for (id, record) in self.executed_units() {
            if record.owner != owner
                || !live(record)
                || record.construction.is_some()
                || self.training.jobs.iter().any(|job| job.producer == id)
            {
                continue;
            }
            let list = profile(&record.definition, "Researches", "upgradeList")
                .map_err(|e| self.research_error(e))?;
            let mut researches = false;
            for token in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                if token.len() != 4 || !token.bytes().all(|b| b.is_ascii_graphic()) {
                    return Err(self.research_error("malformed Researches rawcode"));
                }
                researches |= token.as_bytes() == upgrade.to_be_bytes();
            }
            if researches {
                if record.paused_since.is_some() {
                    return Err(
                        self.research_error("research at a paused building is not established")
                    );
                }
                chosen = Some(id);
                break;
            }
        }
        let Some(producer) = chosen else {
            return Ok(());
        };
        if self.training.jobs.len() >= self.inner.max_handles {
            return Err(self.research_error("training job budget exhausted"));
        }
        self.training
            .jobs
            .try_reserve(1)
            .map_err(|_| self.research_error("training job capacity"))?;
        let now = self
            .timer_game_time()
            .filter(|n| n.is_finite() && *n >= 0.)
            .ok_or_else(|| self.research_error("training clock is not admitted"))?;
        let Some(debit) = self.prepare_training_debit(owner, gold, lumber, 0, 0)? else {
            return Ok(());
        };
        self.training.jobs.push(Training {
            producer,
            owner,
            guard: None,
            active: true,
            duration: seconds,
            gold,
            lumber,
            kind: upgrade,
            research: Some(level),
            structure_upgrade: false,
            food: 0,
            finish: now + f64::from(seconds),
            through: now,
            pause: None,
        });
        self.commit_training_debit(debit);
        Ok(())
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

    /// Unit types `owner` has queued for training or structure upgrade, active or waiting.
    /// Research jobs are excluded: they carry an upgrade id, not a unit type.
    pub(super) fn pending_production_kinds(&self, owner: HandleId) -> impl Iterator<Item = i32> {
        self.training
            .jobs
            .iter()
            .filter(move |j| j.owner == owner && j.research.is_none())
            .map(|j| j.kind)
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
                let events = self.prepare_job_events(&job, job.producer, "START")?;
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
            if job.research.is_some() || job.structure_upgrade {
                // Research levels and structure replacements run after this tick's
                // regeneration (`complete_research`, `complete_structure_upgrade`).
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

    /// Applies the research that finished by `now`. It runs once the tick's regeneration has
    /// reached `now`, so the level change integrates no motion: an upgrade effect on a moving
    /// unit rebases its regeneration clock.
    pub(super) fn complete_research(&mut self, now: f64) -> Result<(), Error> {
        let mut index = 0;
        while index < self.training.jobs.len() {
            let job = self.training.jobs[index];
            let Some(level) = job.research else {
                index += 1;
                continue;
            };
            let paused = self
                .executed_unit(job.producer)
                .map_err(|e| self.inner.error(NATIVE, e.failure))?
                .paused_since
                .is_some();
            if !job.active || paused || job.finish > now {
                index += 1;
                continue;
            }
            self.set_researched_with_effects(&[
                Value::Handle(job.owner),
                Value::Int(job.kind),
                Value::Int(level),
            ])?;
            self.training.jobs.remove(index);
        }
        Ok(())
    }

    /// Replaces a building that finished its structure upgrade by `now`.
    pub(super) fn complete_structure_upgrade(&mut self, now: f64) -> Result<(), Error> {
        let mut index = 0;
        while index < self.training.jobs.len() {
            let job = self.training.jobs[index];
            if !job.structure_upgrade {
                index += 1;
                continue;
            }
            let record = self
                .executed_unit(job.producer)
                .map_err(|e| self.inner.error(NATIVE, e.failure))?;
            if !job.active || record.paused_since.is_some() || job.finish > now {
                index += 1;
                continue;
            }
            let upgrade = self.prepare_upgrade_replacement(job.producer, job.kind)?;
            self.commit_upgrade_replacement(job.producer, upgrade);
            self.training.jobs.remove(index);
        }
        Ok(())
    }

    fn prepare_upgrade_vitals(
        &self,
        producer: HandleId,
        definition: &Definition,
    ) -> Result<super::unit_vitals::Vitals, Error> {
        let record = self.executed_unit(producer)?;
        let old = record
            .vitals
            .ok_or_else(|| self.training_error("upgrade requires live pools"))?;
        let mut next = self
            .prepare_unit_vitals(definition, record.health_handicap)?
            .ok_or_else(|| self.training_error("upgrade requires target pools"))?;
        for pools in [&old, &next] {
            if let Some(problem) = Self::projected_pool_problem(pools) {
                return Err(self.training_error(problem));
            }
        }
        if old.max_mana != 0. || next.max_mana != 0. {
            return Err(self.training_error("upgrade mana transition is not qualified"));
        }
        next.life = if old.max_life == next.max_life {
            old.life
        } else if old.life == old.max_life {
            next.max_life
        } else {
            let rounding = self
                .health_rounding()
                .ok_or_else(|| self.training_error("upgrade life rounding is not admitted"))?;
            let life = rounding.round(next.max_life * (old.life / old.max_life));
            if old.life > 0. {
                life.max(1.).min(next.max_life)
            } else {
                0.
            }
        };
        Ok(next)
    }

    fn prepare_upgrade_replacement(
        &mut self,
        producer: HandleId,
        kind: i32,
    ) -> Result<UpgradeReplacement, Error> {
        let definition = self.resolve_form_definition(kind.to_be_bytes())?;
        let vitals = self.prepare_upgrade_vitals(producer, &definition)?;
        let form = if self.has_unit_model_loader() {
            self.prepare_form(
                producer,
                definition,
                super::unit_creation::FormChange::Replacement,
            )?
        } else {
            let activity = self.prepare_unit_activity(&definition)?;
            let combat = self.prepare_unit_combat(&definition)?;
            super::unit_morph::Form {
                definition,
                animation: None,
                activity,
                combat,
            }
        };
        let abilities = self.prepare_unit_abilities(&form.definition)?;
        let placement = self.prepare_morph_placement(producer, &form.definition)?;
        let record = self.executed_unit(producer)?;
        let revision = record
            .definition_revision
            .checked_add(1)
            .ok_or_else(|| self.training_error("definition revision exhausted"))?;
        let radius = Self::catalog_collision_radius(&form.definition);
        let mut target = record.clone();
        target.definition = form.definition.clone();
        target.animation = form.animation.clone();
        target.abilities = abilities.clone();
        target.vitals = Some(vitals);
        let decay = self.prepare_unit_decay(&target)?;
        Ok(UpgradeReplacement {
            form,
            abilities,
            vitals,
            decay,
            placement,
            revision,
            radius,
            death_animation: None,
        })
    }

    fn commit_upgrade_replacement(&mut self, producer: HandleId, upgrade: UpgradeReplacement) {
        let Entry::Unit { record } = self
            .inner
            .entry_mut(producer)
            .expect("prepared upgrade unit")
        else {
            unreachable!("prepared upgrade unit")
        };
        record.definition = upgrade.form.definition;
        record.definition_revision = upgrade.revision;
        record.collision_radius = upgrade.radius;
        record.animation = upgrade.form.animation;
        record.abilities = upgrade.abilities;
        record.vitals = Some(upgrade.vitals);
        record.decay = upgrade.decay;
        if let (Some(current), Some(profile)) = (&mut record.activity, &upgrade.form.activity) {
            current.replace_form_profile(profile);
        }
        if let (Some(current), Some(profile)) = (&mut record.combat, &upgrade.form.combat) {
            current.replace_form_profile(profile);
        }
        self.unit_placement
            .as_mut()
            .expect("prepared upgrade placement")
            .commit(producer, upgrade.placement);
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

    /// Research jobs raise no training events: the research events are not admitted.
    fn prepare_job_events(
        &mut self,
        job: &Training,
        producer: HandleId,
        phase: &str,
    ) -> Result<Vec<TrainingEvent>, Error> {
        if job.research.is_some() || job.structure_upgrade {
            return Ok(Vec::new());
        }
        self.prepare_training_events(producer, phase)
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

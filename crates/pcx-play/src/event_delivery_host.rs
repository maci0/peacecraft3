//! Fired trigger events and their event-response context. Engine changes
//! (region membership, deaths) queue events; the session delivers them at
//! script boundaries in firing order: filter, enabled check, conditions, then
//! actions as one thread. Player-state changes admitted by
//! `with_player_state_events` deliver before `SetPlayerState` returns,
//! including nested mutations inside trigger callbacks. Other events defer to
//! the next boundary. Player-state callback ordering is covered by the native
//! nested-mutation and sleeping-action observations.
use super::{Entry, Error, Failure, ObjectHost, TriggerDelivery};
use pcx_script::{HandleId, Value, value::FunctionId};
use std::collections::{BTreeSet, VecDeque};

/// Event responses readable by the delivered thread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventSubject {
    pub unit: Option<HandleId>,
    pub item: Option<HandleId>,
    /// Killing or attacking unit.
    pub source: Option<HandleId>,
    pub destructable: Option<HandleId>,
    pub dialog: Option<HandleId>,
    pub button: Option<HandleId>,
    pub player: Option<HandleId>,
    /// Player state whose change fired the event (`GetEventPlayerState`).
    pub player_state: Option<HandleId>,
    /// Target of the spell whose effect fired the event (`GetSpellTargetUnit`).
    pub spell_target: Option<HandleId>,
    /// Shop sale in progress: the seller and the sold item, set only by a queued sale.
    pub sale: Option<SaleContext>,
    pub summon: Option<SummonContext>,
    pub hero: Option<HeroEvent>,
    pub trained: Option<HandleId>,
    pub trained_type: Option<i32>,
}

/// Units captured when a summoning effect creates a unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SummonContext {
    pub summoner: HandleId,
    pub summoned: HandleId,
}

/// Hero-specific responses captured when the change fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeroEvent {
    Level,
    Skill { ability: i32, level: i32 },
}

/// One completed sale, delivered to its sell-event triggers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaleContext {
    pub shop: HandleId,
    pub item: HandleId,
}

#[derive(Debug)]
pub struct FiredEvent {
    pub trigger: HandleId,
    pub filter: Option<FunctionId>,
    pub subject: EventSubject,
    event: Option<HandleId>,
}

#[derive(Default)]
pub(super) struct Delivery {
    queue: VecDeque<FiredEvent>,
    /// Events queued by a synchronous mutation, delivered before its native returns.
    synchronous: Option<VecDeque<FiredEvent>>,
    pub(super) subject: EventSubject,
    active: bool,
    previous: Vec<DeliveryContext>,
    /// Region registrations by arena event handle, with the units inside.
    region_members: BTreeSet<(HandleId, HandleId)>,
    /// Registrations whose membership was initialized without firing.
    region_known: BTreeSet<HandleId>,
}

struct DeliveryContext {
    trigger: Option<HandleId>,
    event: Option<HandleId>,
    subject: EventSubject,
    active: bool,
}

const RESPONSES: [&str; 25] = [
    "GetTrainedUnit",
    "GetTrainedUnitType",
    "GetSummoningUnit",
    "GetSummonedUnit",
    "GetLevelingUnit",
    "GetLearningUnit",
    "GetLearnedSkill",
    "GetLearnedSkillLevel",
    "GetSpellTargetUnit",
    "GetTriggerUnit",
    "GetSellingUnit",
    "GetSoldItem",
    "GetManipulatedItem",
    "GetManipulatingUnit",
    "GetEnteringUnit",
    "GetLeavingUnit",
    "GetDyingUnit",
    "GetKillingUnit",
    "GetAttacker",
    "GetClickedDialog",
    "GetClickedButton",
    "GetTriggerPlayer",
    "GetEventPlayerState",
    "GetTriggerWidget",
    "GetTriggerDestructable",
];

impl ObjectHost {
    pub(crate) fn has_queued_training_events(&self) -> bool {
        self.events
            .queue
            .iter()
            .any(|event| event.subject.trained_type.is_some())
    }
    pub(super) fn reserve_event_delivery(&mut self, count: usize) -> Result<(), Error> {
        let queue = match &mut self.events.synchronous {
            Some(queue) => queue,
            None => &mut self.events.queue,
        };
        queue
            .try_reserve(count)
            .map_err(|_| self.inner.error("event delivery", Failure::Capacity))
    }
    /// Route events queued by the current mutation into a synchronous batch.
    pub(super) fn begin_synchronous_delivery(&mut self) {
        self.events.synchronous = Some(VecDeque::new());
    }
    pub(super) fn take_synchronous_delivery(&mut self) -> Option<VecDeque<FiredEvent>> {
        self.events.synchronous.take()
    }
    /// Capacity of the queue a mutation would reserve in, for allocation tests.
    #[cfg(test)]
    pub(crate) fn event_queue_capacity(&self) -> usize {
        match &self.events.synchronous {
            Some(queue) => queue.capacity(),
            None => self.events.queue.capacity(),
        }
    }
    pub(super) fn check_trigger_player_response(&self, source: &str) -> Result<(), Error> {
        let name = "GetTriggerPlayer";
        if self.inner.program.natives().get(name).is_none()
            && self.inner.program.natives().ambiguous(name).is_none()
        {
            return Ok(());
        }
        Self::contract_in(
            &self.inner,
            name,
            &[],
            "player",
            true,
            Some("player"),
            false,
        )?;
        if self
            .inner
            .program
            .natives()
            .get(name)
            .is_none_or(|n| n.source != source)
        {
            return Err(self.inner.error(name, Failure::Source));
        }
        Ok(())
    }
    pub(super) fn reserve_dialog_event_delivery(
        &mut self,
        count: usize,
        limit: usize,
    ) -> Result<(), Error> {
        let pending = self
            .events
            .queue
            .iter()
            .filter(|event| event.subject.dialog.is_some())
            .count();
        if pending.checked_add(count).is_none_or(|n| n > limit) {
            return Err(self.inner.error(
                "dialog button input",
                Failure::Budget("pending dialog events"),
            ));
        }
        self.events
            .queue
            .try_reserve(count)
            .map_err(|_| self.inner.error("event delivery", Failure::Capacity))
    }

    /// Resolves an event filter the way `queue_event` does, so a caller can check it before a write.
    pub(super) fn resolve_event_filter(
        &self,
        filter: Option<HandleId>,
    ) -> Result<Option<FunctionId>, Error> {
        Ok(match filter {
            None => None,
            Some(id) => Some(match self.inner.entry(id)? {
                Entry::Condition { .. } => self.condition_callback(id)?.function,
                Entry::Filter { .. } => self.filter_callback(id)?.function,
                _ => return Err(self.inner.error("event delivery", Failure::WrongFamily)),
            }),
        })
    }

    /// Pushes an event into capacity that `reserve_event_delivery` already reserved. The push
    /// cannot allocate, so it has no failure path.
    pub(crate) fn push_reserved_event(
        &mut self,
        trigger: HandleId,
        filter: Option<FunctionId>,
        subject: EventSubject,
        event: Option<HandleId>,
    ) {
        let fired = FiredEvent {
            trigger,
            filter,
            subject,
            event,
        };
        match &mut self.events.synchronous {
            Some(queue) => queue.push_back(fired),
            None => self.events.queue.push_back(fired),
        }
    }

    pub(super) fn queue_event(
        &mut self,
        trigger: HandleId,
        filter: Option<HandleId>,
        subject: EventSubject,
        event: Option<HandleId>,
    ) -> Result<(), Error> {
        let filter = match filter {
            None => None,
            Some(id) => Some(match self.inner.entry(id)? {
                Entry::Condition { .. } => self.condition_callback(id)?.function,
                Entry::Filter { .. } => self.filter_callback(id)?.function,
                _ => return Err(self.inner.error("event delivery", Failure::WrongFamily)),
            }),
        };
        self.events
            .queue
            .try_reserve(1)
            .map_err(|_| self.inner.error("event delivery", Failure::Capacity))?;
        self.events.queue.push_back(FiredEvent {
            trigger,
            filter,
            subject,
            event,
        });
        Ok(())
    }

    pub fn take_fired_event(&mut self) -> Option<FiredEvent> {
        self.events.queue.pop_front()
    }

    /// Install the event context. `None` skips a destroyed or disabled trigger.
    pub fn begin_fired_event(
        &mut self,
        fired: &FiredEvent,
    ) -> Result<Option<TriggerDelivery>, Error> {
        self.begin_fired_event_context(fired, false)
    }

    pub(super) fn begin_player_state_fired_event(
        &mut self,
        fired: &FiredEvent,
    ) -> Result<Option<TriggerDelivery>, Error> {
        self.begin_fired_event_context(fired, true)
    }

    fn begin_fired_event_context(
        &mut self,
        fired: &FiredEvent,
        nested: bool,
    ) -> Result<Option<TriggerDelivery>, Error> {
        let name = "event delivery";
        if !nested
            && (self.events.active
                || self
                    .trigger_runtime
                    .as_ref()
                    .is_some_and(|s| s.active.is_some()))
        {
            return Err(self
                .inner
                .error(name, Failure::TriggerState("unfinished trigger")));
        }
        if matches!(self.inner.entry(fired.trigger)?, Entry::DeadTrigger)
            || self.trigger_enabled(fired.trigger)? == Some(false)
        {
            // No delivery means no finish call, so the powerup settles here.
            self.settle_powerup_item(fired.subject.item);
            return Ok(None);
        }
        let delivery = self.trigger_delivery(fired.trigger, name)?;
        self.events
            .previous
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        let state = self
            .trigger_runtime
            .as_mut()
            .ok_or_else(|| self.inner.error(name, Failure::TriggerState("no runtime")))?;
        self.events.previous.push(DeliveryContext {
            trigger: state.active,
            event: state.event_context,
            subject: self.events.subject,
            active: self.events.active,
        });
        state.active = Some(fired.trigger);
        state.event_context = fired.event;
        self.events.subject = fired.subject;
        self.events.active = true;
        Ok(Some(delivery))
    }

    pub fn finish_fired_event(&mut self) {
        let previous = self.events.previous.pop();
        if let Some(state) = &mut self.trigger_runtime {
            state.active = previous.as_ref().and_then(|context| context.trigger);
            state.event_context = previous.as_ref().and_then(|context| context.event);
        }
        let item = self.events.subject.item;
        self.events.subject = previous
            .as_ref()
            .map_or_else(EventSubject::default, |context| context.subject);
        self.events.active = previous.is_some_and(|context| context.active);
        self.settle_powerup_item(item);
    }

    /// Consumes a claimed powerup once no pickup event for that item is still queued.
    pub(crate) fn settle_powerup_item(&mut self, item: Option<HandleId>) {
        let Some(item) = item else { return };
        if self
            .events
            .queue
            .iter()
            .any(|e| e.subject.item == Some(item))
        {
            return;
        }
        // A claimed handle was validated as an item when its grant was staged.
        if let Ok(Entry::Item { record }) = self.inner.entry_mut(item)
            && record.claimed
        {
            record.consumed = true;
        }
    }

    pub(super) fn event_response_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if matches!(name, "GetClickedDialog" | "GetClickedButton") && self.dialogs.is_none() {
            return None;
        }
        if !RESPONSES.contains(&name) || !args.is_empty() {
            return None;
        }
        let native = self.inner.program.natives().get(name)?;
        if matches!(name, "GetTriggerWidget" | "GetTriggerDestructable") {
            let source = self
                .triggers
                .as_ref()
                .and_then(|admitted| admitted.death_event_source.as_deref())?;
            self.destructable_death_event_handle()?;
            if native.source != source {
                return Some(Err(self.inner.error(name, Failure::Source)));
            }
        }
        let subject = self.events.subject;
        if matches!(name, "GetTrainedUnit" | "GetTrainedUnitType") {
            let result = if name == "GetTrainedUnit" {
                "unit"
            } else {
                "integer"
            };
            if let Err(error) = Self::contract(&self.inner, name, &[], result, true, None) {
                return Some(Err(error));
            }
            let source = self.unit_declaration().ok()?.0;
            if native.source != source {
                return Some(Err(self.inner.error(name, Failure::Source)));
            }
            if name == "GetTrainedUnitType" {
                return Some(Ok(Value::Int(subject.trained_type.unwrap_or(0))));
            }
        }
        if matches!(
            name,
            "GetLevelingUnit" | "GetLearningUnit" | "GetLearnedSkill" | "GetLearnedSkillLevel"
        ) && !self.has_hero_skills()
        {
            return None;
        }
        if matches!(name, "GetLearnedSkill" | "GetLearnedSkillLevel") {
            return Some(Ok(Value::Int(match subject.hero {
                Some(HeroEvent::Skill { ability, level }) => {
                    if name == "GetLearnedSkill" {
                        ability
                    } else {
                        level
                    }
                }
                _ => 0,
            })));
        }
        if name == "GetTriggerPlayer" {
            let source = &self.inner.program.natives().get("CreateTimer")?.source;
            if let Err(error) = self.check_trigger_player_response(source) {
                return Some(Err(error));
            }
            if self.events.active && subject.player.is_none() {
                return None;
            }
        }
        let value = match name {
            "GetTrainedUnit" => subject.trained,
            "GetLevelingUnit" => matches!(subject.hero, Some(HeroEvent::Level))
                .then_some(subject.unit)
                .flatten(),
            "GetLearningUnit" => matches!(subject.hero, Some(HeroEvent::Skill { .. }))
                .then_some(subject.unit)
                .flatten(),
            "GetKillingUnit" | "GetAttacker" => subject.source,
            "GetManipulatedItem" => subject.item,
            "GetSellingUnit" => subject.sale.map(|sale| sale.shop),
            "GetSoldItem" => subject.sale.map(|sale| sale.item),
            "GetSummoningUnit" => subject.summon.map(|summon| summon.summoner),
            "GetSummonedUnit" => subject.summon.map(|summon| summon.summoned),
            "GetManipulatingUnit" => subject.unit,
            "GetClickedDialog" => subject.dialog,
            "GetClickedButton" => subject.button,
            "GetTriggerPlayer" => subject.player,
            "GetEventPlayerState" => subject.player_state,
            "GetTriggerWidget" => subject.unit.or(subject.destructable),
            "GetTriggerDestructable" => subject.destructable,
            "GetSpellTargetUnit" => subject.spell_target,
            _ if subject.unit.is_some() => subject.unit,
            // Fall through to range delivery, which owns its own responses.
            "GetTriggerUnit" | "GetEnteringUnit" if self.range_context().is_some() => return None,
            _ => None,
        };
        Some(Ok(value.map_or(Value::Null, Value::Handle)))
    }

    /// Fire enter/leave events from current membership of shown, live units in
    /// each registered region. A region contains a point inside any of its
    /// rectangles (native cell cover unqualified).
    pub fn poll_region_events(&mut self, work: &mut usize) -> Result<usize, Error> {
        let name = "region delivery";
        if self.regions.is_none() || self.trigger_runtime.is_none() {
            return Ok(0);
        }
        let mut registrations = Vec::new();
        for (index, entry) in self.inner.arena.iter().enumerate() {
            if let Entry::RegisteredRegionEvent(record) = entry {
                let id = self.inner.id(index, name)?;
                registrations.push((id, *record));
            }
        }
        if registrations.is_empty() {
            return Ok(0);
        }
        let mut units = Vec::new();
        for (id, unit) in self.executed_units() {
            if let (true, Some(position)) = (unit.shown, unit.position)
                && unit.vitals.is_none_or(|v| v.life > 0.0)
            {
                units.push((id, position));
            }
        }
        let mut members = BTreeSet::new();
        let mut fired = Vec::new();
        for (event, record) in &registrations {
            let Entry::Region { additions } = self.inner.entry(record.region)? else {
                return Err(self.inner.error(name, Failure::WrongFamily));
            };
            for &(unit, [x, y]) in &units {
                *work = work
                    .checked_sub(1)
                    .ok_or_else(|| self.inner.error(name, Failure::Budget("region scan")))?;
                let inside = additions.iter().any(|r| {
                    let [min_x, min_y, max_x, max_y] = r.bounds;
                    (min_x..=max_x).contains(&x) && (min_y..=max_y).contains(&y)
                });
                if inside {
                    members.insert((*event, unit));
                }
            }
        }
        for (event, record) in &registrations {
            // ponytail: membership seeds silently at a registration's first poll,
            // so units already inside at registration do not fire; units created
            // between registration and that poll are missed (none in Human01's main).
            if self.events.region_known.insert(*event) {
                continue;
            }
            let changed: Vec<HandleId> = match record.kind {
                super::RegionEventKind::Enter => members
                    .iter()
                    .filter(|m| m.0 == *event && !self.events.region_members.contains(m))
                    .map(|m| m.1)
                    .collect(),
                super::RegionEventKind::Leave => self
                    .events
                    .region_members
                    .iter()
                    .filter(|m| m.0 == *event && !members.contains(m))
                    .map(|m| m.1)
                    .collect(),
            };
            for unit in changed {
                fired.push((record.trigger, record.filter, unit));
            }
        }
        self.events.region_members = members;
        let count = fired.len();
        for (trigger, filter, unit) in fired {
            let subject = EventSubject {
                unit: Some(unit),
                ..EventSubject::default()
            };
            self.queue_event(trigger, filter, subject, None)?;
        }
        Ok(count)
    }
}

//! Fired trigger events and their event-response context. Engine changes
//! (region membership, deaths) queue events; the session delivers them at
//! script boundaries in firing order: filter, enabled check, conditions, then
//! actions as one thread. Deferring delivery to the next boundary is a
//! recreation choice; the original's synchronous nesting is unqualified.
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
    /// Target of the spell whose effect fired the event (`GetSpellTargetUnit`).
    pub spell_target: Option<HandleId>,
    /// Shop sale in progress: the seller and the sold item, set only by a queued sale.
    pub sale: Option<SaleContext>,
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
    pub(super) subject: EventSubject,
    active: bool,
    /// Region registrations by arena event handle, with the units inside.
    region_members: BTreeSet<(HandleId, HandleId)>,
    /// Registrations whose membership was initialized without firing.
    region_known: BTreeSet<HandleId>,
}

const RESPONSES: [&str; 14] = [
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
];

impl ObjectHost {
    pub(super) fn reserve_event_delivery(&mut self, count: usize) -> Result<(), Error> {
        self.events
            .queue
            .try_reserve(count)
            .map_err(|_| self.inner.error("event delivery", Failure::Capacity))
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
        let name = "event delivery";
        if self.events.active
            || self
                .trigger_runtime
                .as_ref()
                .is_some_and(|s| s.active.is_some())
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
        let state = self
            .trigger_runtime
            .as_mut()
            .ok_or_else(|| self.inner.error(name, Failure::TriggerState("no runtime")))?;
        state.active = Some(fired.trigger);
        state.event_context = fired.event;
        self.events.subject = fired.subject;
        self.events.active = true;
        Ok(Some(delivery))
    }

    pub fn finish_fired_event(&mut self) {
        if let Some(state) = &mut self.trigger_runtime {
            state.active = None;
            state.event_context = None;
        }
        let item = self.events.subject.item;
        self.events.subject = EventSubject::default();
        self.events.active = false;
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
        self.inner.program.natives().get(name)?;
        let subject = self.events.subject;
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
            "GetKillingUnit" | "GetAttacker" => subject.source,
            "GetManipulatedItem" => subject.item,
            "GetSellingUnit" => subject.sale.map(|sale| sale.shop),
            "GetSoldItem" => subject.sale.map(|sale| sale.item),
            "GetManipulatingUnit" => subject.unit,
            "GetClickedDialog" => subject.dialog,
            "GetClickedButton" => subject.button,
            "GetTriggerPlayer" => subject.player,
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

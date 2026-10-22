//! Source-bound death subscriptions; no death detection or event delivery.
use super::super::EventSubject;
use super::super::{PlayerStateDelivery, PlayerStateFrame};
use super::{Entry, Error, Failure, ObjectHost, loaded_type};
use pcx_script::{HandleId, ScriptCall, Value};

pub(super) const REGISTER: &str = "TriggerRegisterDeathEvent";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeathEventRegistration {
    pub trigger: HandleId,
    pub widget: HandleId,
}

impl ObjectHost {
    pub fn with_death_events(mut self, max_records: usize) -> Result<Self, Error> {
        if self.admitted_triggers()?.death_event_limit.is_some() {
            return Err(self.inner.error(
                REGISTER,
                Failure::TriggerState("death events already admitted"),
            ));
        }
        std::alloc::Layout::array::<DeathEventRegistration>(max_records)
            .map_err(|_| self.inner.error(REGISTER, Failure::Capacity))?;
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "widget"],
            "event",
            false,
            Some("event"),
        )?;
        let source = &self
            .inner
            .program
            .natives()
            .get(REGISTER)
            .expect("checked contract")
            .source;
        for name in ["CreateTrigger", "CreateUnit", "CreateDestructable"] {
            if let Some(native) = self.inner.program.natives().get(name)
                && &native.source != source
            {
                return Err(self.inner.error(REGISTER, Failure::Source));
            }
        }
        for (name, parent) in [("widget", "agent"), ("unit", "widget")] {
            loaded_type(&self.inner, REGISTER, name, parent, source)?;
        }
        if self
            .inner
            .program
            .types()
            .any(|ty| ty.name == "destructable")
        {
            loaded_type(&self.inner, REGISTER, "destructable", "widget", source)?;
        }
        self.triggers
            .as_mut()
            .expect("admitted above")
            .death_event_limit = Some(max_records);
        Ok(self)
    }

    pub fn retained_death_events(&self) -> Result<usize, Error> {
        let a = self.admitted_triggers()?;
        if a.death_event_limit.is_none() {
            return Err(self
                .inner
                .error(REGISTER, Failure::TriggerState("death events not admitted")));
        }
        Ok(a.retained_death_events)
    }

    pub fn death_event_registration(&self, id: HandleId) -> Result<DeathEventRegistration, Error> {
        self.retained_death_events()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(REGISTER, e.failure))?
        {
            Entry::RegisteredDeathEvent(record) => Ok(*record),
            _ => Err(self.inner.error(REGISTER, Failure::WrongFamily)),
        }
    }

    pub(super) fn register_death_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |failure| self.inner.error(REGISTER, failure);
        let [Value::Handle(trigger), Value::Handle(widget)] = args else {
            return Err(fail(Failure::TriggerState("expected trigger and widget")));
        };
        if !matches!(
            self.inner.entry(*trigger).map_err(|e| fail(e.failure))?,
            Entry::Trigger { .. }
        ) || !matches!(
            self.inner.entry(*widget).map_err(|e| fail(e.failure))?,
            Entry::Unit { .. } | Entry::Destructable { .. }
        ) {
            return Err(fail(Failure::WrongFamily));
        }
        let a = self.admitted_triggers()?;
        let limit = a
            .death_event_limit
            .ok_or_else(|| fail(Failure::TriggerState("death events not admitted")))?;
        if a.retained_death_events >= limit {
            return Err(fail(Failure::Budget("death event registration")));
        }
        let id = self.inner.allocate(
            Entry::RegisteredDeathEvent(DeathEventRegistration {
                trigger: *trigger,
                widget: *widget,
            }),
            REGISTER,
        )?;
        self.triggers
            .as_mut()
            .expect("admitted above")
            .retained_death_events += 1;
        Ok(Value::Handle(id))
    }
}

impl ObjectHost {
    /// Number of death subscriptions attached to `widget`, used to reserve
    /// delivery capacity before the life mutation commits.
    pub(crate) fn widget_death_event_count(&self, widget: HandleId) -> usize {
        self.inner
            .arena
            .iter()
            .filter(|entry| {
                matches!(
                    entry,
                    Entry::RegisteredDeathEvent(record) if record.widget == widget
                )
            })
            .count()
    }

    /// Queues the death events `widget_death_event_count` reserved. The push
    /// itself cannot allocate; synchronous mutation batches keep their routing.
    pub(crate) fn queue_widget_death_events(&mut self, widget: HandleId) -> Result<(), Error> {
        for index in 0..self.inner.arena.len() {
            let Entry::RegisteredDeathEvent(record) = &self.inner.arena[index] else {
                continue;
            };
            if record.widget != widget {
                continue;
            }
            let trigger = record.trigger;
            let event = self.inner.id(index, REGISTER)?;
            self.push_reserved_event(
                trigger,
                None,
                EventSubject {
                    destructable: Some(widget),
                    ..EventSubject::default()
                },
                Some(event),
            );
        }
        Ok(())
    }

    /// Intercept `KillDestructable` / `SetDestructableLife` when the widget has
    /// death subscriptions, so callbacks run before the native returns.
    pub(crate) fn begin_native_death_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, Error> {
        if !matches!(name, "KillDestructable" | "SetDestructableLife") {
            return Ok(None);
        }
        let Some(Value::Handle(widget)) = args.first() else {
            return Ok(None);
        };
        if self.widget_death_event_count(*widget) == 0 {
            return Ok(None);
        }
        if self.death_frames.len() >= pcx_script::eval::MAX_CALL_DEPTH as usize {
            return Err(self
                .inner
                .error(name, Failure::Budget("nested destructable callbacks")));
        }
        self.death_frames
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        self.begin_synchronous_delivery();
        let result = self.destructable_native(name, args);
        let Some(events) = self.take_synchronous_delivery() else {
            return Err(self.inner.error(
                "destructable death delivery",
                Failure::TriggerState("synchronous batch disappeared"),
            ));
        };
        let Some(result) = result else {
            return Err(self.inner.error(name, Failure::Signature));
        };
        result?;
        if events.is_empty() {
            return Ok(Some(ScriptCall::Return(Value::Null)));
        }
        self.death_frames.push(PlayerStateFrame {
            events,
            active: false,
            spawn_depth: 0,
            callback_depth: 0,
            conditions: None,
        });
        self.begin_next_death_event().map(Some)
    }

    fn begin_next_death_event(&mut self) -> Result<ScriptCall, Error> {
        loop {
            let Some(event) = self
                .death_frames
                .last_mut()
                .and_then(|frame| frame.events.pop_front())
            else {
                self.death_frames.pop();
                return Ok(ScriptCall::Return(Value::Null));
            };
            let filter = event.filter;
            let Some(delivery) = self.begin_player_state_fired_event(&event)? else {
                continue;
            };
            self.death_frames
                .last_mut()
                .expect("active death call")
                .active = true;
            let mut conditions = Vec::new();
            let count = usize::from(filter.is_some()) + delivery.conditions.len();
            conditions.try_reserve_exact(count).map_err(|_| {
                self.inner
                    .error("destructable death delivery", Failure::Capacity)
            })?;
            conditions.extend(filter);
            conditions.extend(delivery.conditions);
            if conditions.is_empty() {
                if delivery.actions.is_empty() {
                    self.finish_fired_event();
                    self.death_frames
                        .last_mut()
                        .expect("active death call")
                        .active = false;
                    continue;
                }
                let depth = self.spawned_thread_depth();
                self.death_frames
                    .last_mut()
                    .expect("active death call")
                    .spawn_depth = depth;
                self.enter_spawned_thread();
                return Ok(ScriptCall::Spawn(delivery.actions));
            }
            let first = conditions[0];
            let frame = self.death_frames.last_mut().expect("active death call");
            frame.callback_depth = self.callbacks.len();
            frame.conditions = Some(PlayerStateDelivery {
                conditions,
                actions: delivery.actions,
                next: 1,
            });
            return Ok(ScriptCall::Call(first));
        }
    }

    pub(crate) fn continue_death_call(&mut self, value: Value) -> Result<ScriptCall, Error> {
        let fail = |reason| self.inner.error("destructable death delivery", reason);
        let Value::Bool(accepted) = value else {
            return Err(fail(Failure::TriggerState(
                "destructable death condition did not return boolean",
            )));
        };
        let Some(frame) = self
            .death_frames
            .last_mut()
            .and_then(|frame| frame.conditions.as_mut())
        else {
            return Err(fail(Failure::TriggerState("no active delivery")));
        };
        if !accepted {
            self.death_frames
                .last_mut()
                .expect("active death call")
                .conditions = None;
            self.finish_fired_event();
            self.death_frames
                .last_mut()
                .expect("active death call")
                .active = false;
            return self.begin_next_death_event();
        }
        if let Some(&next) = frame.conditions.get(frame.next) {
            frame.next += 1;
            return Ok(ScriptCall::Call(next));
        }
        let frame = self
            .death_frames
            .last_mut()
            .expect("active death call")
            .conditions
            .take()
            .expect("checked active delivery");
        if frame.actions.is_empty() {
            self.finish_fired_event();
            self.death_frames
                .last_mut()
                .expect("active death call")
                .active = false;
            self.begin_next_death_event()
        } else {
            let depth = self.spawned_thread_depth();
            self.death_frames
                .last_mut()
                .expect("active death call")
                .spawn_depth = depth;
            self.enter_spawned_thread();
            Ok(ScriptCall::Spawn(frame.actions))
        }
    }

    pub(crate) fn after_death_spawn(&mut self) -> Result<Option<ScriptCall>, Error> {
        let depth = self.spawned_thread_depth();
        if self.death_frames.last().is_none_or(|frame| {
            !frame.active || frame.conditions.is_some() || depth != frame.spawn_depth
        }) {
            return Ok(None);
        }
        self.finish_fired_event();
        self.death_frames
            .last_mut()
            .expect("active death call")
            .active = false;
        self.begin_next_death_event().map(Some)
    }

    pub(crate) fn abort_death_delivery(&mut self) {
        while let Some(frame) = self.death_frames.pop() {
            if frame.active {
                self.finish_fired_event();
            }
        }
    }
}

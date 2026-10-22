//! Source-bound player-state subscriptions. Registration seeds the observed
//! value. The central player-state mutation path queues qualifying changes as
//! they happen; a boundary poll catches effective changes computed outside that
//! path, such as live food.
use super::super::PlayerStateDelivery;
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, ScriptCall, Value};
pub(super) const REGISTER: &str = "TriggerRegisterPlayerStateEvent";

const SET: &str = "SetPlayerState";

impl ObjectHost {
    /// Intercept an admitted `SetPlayerState` so its callbacks run before the
    /// native returns. Events stay routed through the shared delivery context.
    pub(crate) fn begin_synchronous_player_state_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, Error> {
        if name != SET
            || self.delivery_active()
            || self
                .trigger_runtime
                .as_ref()
                .is_some_and(|runtime| runtime.active.is_some())
            || self
                .triggers
                .as_ref()
                .is_none_or(|admitted| admitted.player_state_event_limit.is_none())
        {
            return Ok(None);
        }
        self.begin_synchronous_delivery();
        let result = self.request_player_state(args);
        let Some(events) = self.take_synchronous_delivery() else {
            return Err(self.inner.error(
                "player state delivery",
                Failure::TriggerState("synchronous batch disappeared"),
            ));
        };
        result?;
        if events.is_empty() {
            return Ok(Some(ScriptCall::Return(Value::Null)));
        }
        self.player_state_sync = Some(events);
        self.begin_next_player_state_event().map(Some)
    }

    fn begin_next_player_state_event(&mut self) -> Result<ScriptCall, Error> {
        loop {
            let Some(event) = self
                .player_state_sync
                .as_mut()
                .and_then(|events| events.pop_front())
            else {
                self.player_state_sync = None;
                self.player_state_sync_active = false;
                return Ok(ScriptCall::Return(Value::Null));
            };
            let filter = event.filter;
            let Some(delivery) = self.begin_fired_event(&event)? else {
                continue;
            };
            self.player_state_sync_active = true;
            let mut conditions = Vec::new();
            let count = usize::from(filter.is_some()) + delivery.conditions.len();
            conditions
                .try_reserve_exact(count)
                .map_err(|_| self.inner.error("player state delivery", Failure::Capacity))?;
            conditions.extend(filter);
            conditions.extend(delivery.conditions);
            if conditions.is_empty() {
                if delivery.actions.is_empty() {
                    self.finish_fired_event();
                    self.player_state_sync_active = false;
                    continue;
                }
                self.player_state_spawn_depth = self.spawned_thread_depth();
                self.enter_spawned_thread();
                return Ok(ScriptCall::Spawn(delivery.actions));
            }
            let first = conditions[0];
            self.player_state_callback_depth = self.callbacks.len();
            self.player_state_conditions = Some(PlayerStateDelivery {
                conditions,
                actions: delivery.actions,
                next: 1,
            });
            return Ok(ScriptCall::Call(first));
        }
    }

    pub(crate) fn continue_player_state_call(&mut self, value: Value) -> Result<ScriptCall, Error> {
        let fail = |reason| self.inner.error("player state delivery", reason);
        let Value::Bool(accepted) = value else {
            return Err(fail(Failure::TriggerState(
                "player state condition did not return boolean",
            )));
        };
        let Some(frame) = self.player_state_conditions.as_mut() else {
            return Err(fail(Failure::TriggerState("no active delivery")));
        };
        if !accepted {
            self.player_state_conditions = None;
            self.finish_fired_event();
            self.player_state_sync_active = false;
            return self.begin_next_player_state_event();
        }
        if let Some(&next) = frame.conditions.get(frame.next) {
            frame.next += 1;
            return Ok(ScriptCall::Call(next));
        }
        let frame = self
            .player_state_conditions
            .take()
            .expect("checked active delivery");
        if frame.actions.is_empty() {
            self.finish_fired_event();
            self.player_state_sync_active = false;
            self.begin_next_player_state_event()
        } else {
            self.player_state_spawn_depth = self.spawned_thread_depth();
            self.enter_spawned_thread();
            Ok(ScriptCall::Spawn(frame.actions))
        }
    }

    pub(crate) fn after_player_state_spawn(&mut self) -> Result<Option<ScriptCall>, Error> {
        if !self.player_state_sync_active
            || self.player_state_conditions.is_some()
            || self.spawned_thread_depth() != self.player_state_spawn_depth
        {
            return Ok(None);
        }
        self.finish_fired_event();
        self.player_state_sync_active = false;
        self.begin_next_player_state_event().map(Some)
    }

    pub(crate) fn abort_player_state_delivery(&mut self) {
        self.player_state_conditions = None;
        self.player_state_sync = None;
        if self.player_state_sync_active {
            self.player_state_sync_active = false;
            self.finish_fired_event();
        }
    }
}

const CONVERT_STATE: &str = "ConvertPlayerState";
const CONVERT_LIMIT: &str = "ConvertLimitOp";
const RESPONSE: &str = "GetEventPlayerState";
const LIMIT_OPS: [&str; 6] = [
    "LESS_THAN",
    "LESS_THAN_OR_EQUAL",
    "EQUAL",
    "GREATER_THAN_OR_EQUAL",
    "GREATER_THAN",
    "NOT_EQUAL",
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerStateEventRegistration {
    pub trigger: HandleId,
    pub player: HandleId,
    pub state: HandleId,
    pub state_value: i32,
    pub limitop: HandleId,
    pub limitop_value: i32,
    pub limit: f32,
    /// Most recently observed value; seeded at registration without firing.
    pub last: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Comparison {
    LessThan,
    LessThanOrEqual,
    Equal,
    GreaterThanOrEqual,
    GreaterThan,
    NotEqual,
}

/// One registration whose observed snapshot a prepared change will update, with
/// the decision already made from the pre-change state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PlayerStateEventUpdate {
    index: usize,
    trigger: HandleId,
    state_handle: HandleId,
    registration: HandleId,
    fire: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlayerStateChange {
    player: HandleId,
    current: i32,
    updates: Vec<PlayerStateEventUpdate>,
}

impl Comparison {
    fn at(index: usize) -> Option<Self> {
        Some(match index {
            0 => Self::LessThan,
            1 => Self::LessThanOrEqual,
            2 => Self::Equal,
            3 => Self::GreaterThanOrEqual,
            4 => Self::GreaterThan,
            5 => Self::NotEqual,
            _ => return None,
        })
    }
    fn holds(self, value: i32, limit: f32) -> bool {
        let value = f64::from(value);
        let limit = f64::from(limit);
        match self {
            Self::LessThan => value < limit,
            Self::LessThanOrEqual => value <= limit,
            Self::Equal => value == limit,
            Self::GreaterThanOrEqual => value >= limit,
            Self::GreaterThan => value > limit,
            Self::NotEqual => value != limit,
        }
    }
}

impl ObjectHost {
    /// Admit the selected player-state and limitop conversion domains on the
    /// existing trigger arena. No comparison or delivery is installed here.
    pub fn with_player_state_events(mut self, max_records: usize) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(REGISTER, reason);
        let admitted = self.admitted_triggers()?;
        if admitted.player_state_event_limit.is_some() {
            return Err(fail(Failure::TriggerState(
                "player state events already admitted",
            )));
        }
        std::alloc::Layout::array::<PlayerStateEventRegistration>(max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "player", "playerstate", "limitop", "real"],
            "event",
            false,
            Some("event"),
        )?;
        let program = &self.inner.program;
        let source = &program
            .natives()
            .get(REGISTER)
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        for name in ["CreateTrigger", "Player"] {
            if program
                .natives()
                .get(name)
                .is_none_or(|n| &n.source != source)
            {
                return Err(fail(Failure::Source));
            }
        }
        for (native, kind) in [(CONVERT_STATE, "playerstate"), (CONVERT_LIMIT, "limitop")] {
            Self::contract(&self.inner, native, &["integer"], kind, true, None)?;
            let conversion = self
                .inner
                .conversions
                .get(native)
                .ok_or_else(|| fail(Failure::Signature))?;
            if conversion.kind != kind
                || &conversion.source != source
                || conversion.domain.is_empty()
                || program
                    .natives()
                    .get(native)
                    .is_none_or(|n| &n.source != source)
            {
                return Err(self.inner.error(native, Failure::Source));
            }
        }
        for name in ["playerstate", "limitop"] {
            let mut current = name;
            for _ in 0..=program.types().count() {
                if current == "handle" {
                    break;
                }
                let ty = program
                    .types()
                    .find(|ty| ty.name == current)
                    .ok_or_else(|| fail(Failure::Signature))?;
                if &ty.source != source {
                    return Err(fail(Failure::Source));
                }
                current = &ty.parent;
            }
            if current != "handle" {
                return Err(fail(Failure::Signature));
            }
        }
        if let Some(native) = program.natives().get(RESPONSE) {
            Self::contract(&self.inner, RESPONSE, &[], "playerstate", true, None)?;
            if &native.source != source {
                return Err(self.inner.error(RESPONSE, Failure::Source));
            }
        }
        let mut ops = [0; LIMIT_OPS.len()];
        for (value, name) in ops.iter_mut().zip(LIMIT_OPS) {
            *value = self.source_constant_enumerand(name, "limitop")?;
        }
        if ops
            .iter()
            .enumerate()
            .any(|(index, value)| ops[..index].contains(value))
        {
            return Err(fail(Failure::TriggerState(
                "player state limit operations are not distinct",
            )));
        }
        if self
            .triggers
            .as_mut()
            .ok_or_else(|| fail(Failure::TriggerState("not admitted")))?
            .player_state_registrations
            .try_reserve_exact(max_records)
            .is_err()
        {
            return Err(fail(Failure::Capacity));
        }
        let admitted = self
            .triggers
            .as_mut()
            .ok_or_else(|| fail(Failure::TriggerState("not admitted")))?;
        admitted.player_state_event_limit = Some(max_records);
        admitted.player_state_ops = Some(ops);
        Ok(self)
    }

    pub fn retained_player_state_events(&self) -> Result<usize, Error> {
        let admitted = self.admitted_triggers()?;
        if admitted.player_state_event_limit.is_none() {
            return Err(self.inner.error(
                REGISTER,
                Failure::TriggerState("player state events not admitted"),
            ));
        }
        Ok(admitted.retained_player_state_events)
    }

    pub fn player_state_event_registration(
        &self,
        id: HandleId,
    ) -> Result<PlayerStateEventRegistration, Error> {
        self.retained_player_state_events()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(REGISTER, e.failure))?
        {
            Entry::RegisteredPlayerStateEvent(record) => Ok(*record),
            _ => Err(self.inner.error(REGISTER, Failure::WrongFamily)),
        }
    }

    pub(super) fn register_player_state_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(REGISTER, reason);
        let [
            Value::Handle(trigger),
            Value::Handle(player),
            Value::Handle(state),
            Value::Handle(limitop),
            limit,
        ] = args
        else {
            return Err(fail(Failure::TriggerState(
                "expected trigger, player, player state, limitop and real threshold",
            )));
        };
        if !matches!(
            self.inner.entry(*trigger).map_err(|e| fail(e.failure))?,
            Entry::Trigger { .. }
        ) || !matches!(
            self.inner.entry(*player).map_err(|e| fail(e.failure))?,
            Entry::Player { .. }
        ) {
            return Err(fail(Failure::WrongFamily));
        }
        let state_value = self
            .inner
            .symbolic_value(*state, "playerstate")
            .map_err(|e| fail(e.failure))?;
        if !self
            .inner
            .conversions
            .get(CONVERT_STATE)
            .is_some_and(|c| c.domain.contains(&state_value))
        {
            return Err(fail(Failure::OutsideDomain(state_value)));
        }
        let limitop_value = self
            .inner
            .symbolic_value(*limitop, "limitop")
            .map_err(|e| fail(e.failure))?;
        let admitted = self.admitted_triggers()?;
        let ops = admitted
            .player_state_ops
            .ok_or_else(|| fail(Failure::TriggerState("player state events not admitted")))?;
        if !ops.contains(&limitop_value) {
            return Err(fail(Failure::OutsideDomain(limitop_value)));
        }
        let limit = limit
            .as_real()
            .filter(|v| v.is_finite())
            .ok_or_else(|| fail(Failure::TriggerState("finite real threshold required")))?;
        let last = self
            .player_state_value(*player, *state)?
            .ok_or_else(|| fail(Failure::TriggerState("initial player state is unsupported")))?;
        let admitted = self.admitted_triggers()?;
        let max = admitted
            .player_state_event_limit
            .ok_or_else(|| fail(Failure::TriggerState("player state events not admitted")))?;
        if admitted.retained_player_state_events >= max {
            return Err(fail(Failure::Budget("player state event registration")));
        }
        let record = PlayerStateEventRegistration {
            trigger: *trigger,
            player: *player,
            state: *state,
            state_value,
            limitop: *limitop,
            limitop_value,
            limit,
            last,
        };
        let id = self
            .inner
            .allocate(Entry::RegisteredPlayerStateEvent(record), REGISTER)?;
        let admitted = self.triggers.as_mut().expect("admitted above");
        admitted.retained_player_state_events += 1;
        admitted.player_state_registrations.push(id);
        Ok(Value::Handle(id))
    }

    fn player_state_registration_id(&self, slot: usize) -> Result<HandleId, Error> {
        self.admitted_triggers()?
            .player_state_registrations
            .get(slot)
            .copied()
            .ok_or_else(|| {
                self.inner.error(
                    REGISTER,
                    Failure::TriggerState("registration index is inconsistent"),
                )
            })
    }

    fn player_state_record(&self, id: HandleId) -> Result<PlayerStateEventRegistration, Error> {
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(REGISTER, e.failure))?
        {
            Entry::RegisteredPlayerStateEvent(record) => Ok(*record),
            _ => Err(self.inner.error(REGISTER, Failure::WrongFamily)),
        }
    }

    fn player_state_comparison(
        &self,
        ops: [i32; 6],
        limitop_value: i32,
    ) -> Result<Comparison, Error> {
        let Some(index) = ops.iter().position(|value| *value == limitop_value) else {
            return Err(self
                .inner
                .error(REGISTER, Failure::OutsideDomain(limitop_value)));
        };
        Comparison::at(index).ok_or_else(|| self.inner.error(REGISTER, Failure::Signature))
    }

    /// Reserves the event queue for every registration that would fire at
    /// `current`. Call before changing the effective value so a capacity
    /// failure leaves both the value and the observed snapshots unchanged.
    pub(crate) fn prepare_player_state_change(
        &mut self,
        player: HandleId,
        state: i32,
        current: i32,
    ) -> Result<Option<PlayerStateChange>, Error> {
        self.prepare_player_state_change_reserving(player, state, current, 0)
    }

    /// Like [`Self::prepare_player_state_change`], but reserves one combined
    /// block for `pending` events that will be queued before this change commits.
    /// Reserving separately would only guarantee space for the larger of the two.
    ///
    /// The `pending` block is reserved even when no player-state trigger can
    /// observe the change, so the caller's own event writes never allocate.
    pub(crate) fn prepare_player_state_change_reserving(
        &mut self,
        player: HandleId,
        state: i32,
        current: i32,
        pending: usize,
    ) -> Result<Option<PlayerStateChange>, Error> {
        let ops = self
            .triggers
            .as_ref()
            .and_then(|admitted| admitted.player_state_ops);
        let Some(ops) = ops else {
            self.reserve_event_delivery(pending)?;
            return Ok(None);
        };
        let registrations = self
            .admitted_triggers()?
            .player_state_registrations
            .len();
        let mut updates = Vec::new();
        updates
            .try_reserve(registrations)
            .map_err(|_| self.inner.error(REGISTER, Failure::Capacity))?;
        let mut count: usize = 0;
        for slot in 0..registrations {
            let id = self.player_state_registration_id(slot)?;
            let record = self.player_state_record(id)?;
            if record.player != player || record.state_value != state || record.last == current {
                continue;
            }
            let fire = self
                .player_state_comparison(ops, record.limitop_value)?
                .holds(current, record.limit);
            if fire {
                count += 1;
            }
            let index = usize::try_from(id.token() - 1)
                .map_err(|_| self.inner.error(REGISTER, Failure::Identity))?;
            updates.push(PlayerStateEventUpdate {
                index,
                trigger: record.trigger,
                state_handle: record.state,
                registration: id,
                fire,
            });
        }
        let count = count
            .checked_add(pending)
            .ok_or_else(|| self.inner.error(REGISTER, Failure::Capacity))?;
        self.reserve_event_delivery(count)?;
        Ok(Some(PlayerStateChange {
            player,
            current,
            updates,
        }))
    }

    /// Records the new effective value for every prepared registration and
    /// pushes the events reserved by [`Self::prepare_player_state_change`].
    /// Every fallible lookup and comparison happened during preparation, so the
    /// caller can run this after its other writes without a failure path.
    pub(crate) fn commit_player_state_change(&mut self, change: PlayerStateChange) {
        for update in change.updates {
            if let Entry::RegisteredPlayerStateEvent(stored) =
                &mut self.inner.arena[update.index]
            {
                stored.last = change.current;
            }
            if update.fire {
                self.push_reserved_event(
                    update.trigger,
                    None,
                    super::super::EventSubject {
                        player: Some(change.player),
                        player_state: Some(update.state_handle),
                        ..Default::default()
                    },
                    Some(update.registration),
                );
            }
        }
    }

    /// Queue one event per registration whose effective value changed outside
    /// the central mutation path and whose selected comparison now holds.
    pub fn poll_player_state_events(&mut self, work: &mut usize) -> Result<usize, Error> {
        let (ops, registrations) = {
            let Some(admitted) = self.triggers.as_ref() else {
                return Ok(0);
            };
            let Some(ops) = admitted.player_state_ops else {
                return Ok(0);
            };
            (ops, admitted.player_state_registrations.len())
        };
        let mut fired = 0;
        for slot in 0..registrations {
            *work = work.checked_sub(1).ok_or_else(|| {
                self.inner
                    .error(REGISTER, Failure::Budget("player state scan"))
            })?;
            let id = self.player_state_registration_id(slot)?;
            let record = self.player_state_record(id)?;
            let current = self
                .player_state_value(record.player, record.state)?
                .ok_or_else(|| {
                    self.inner.error(
                        REGISTER,
                        Failure::TriggerState("observed player state is unsupported"),
                    )
                })?;
            if current == record.last {
                continue;
            }
            let comparison = self.player_state_comparison(ops, record.limitop_value)?;
            if comparison.holds(current, record.limit) {
                self.queue_event(
                    record.trigger,
                    None,
                    super::super::EventSubject {
                        player: Some(record.player),
                        player_state: Some(record.state),
                        ..Default::default()
                    },
                    Some(id),
                )?;
                fired += 1;
            }
            match self.inner.entry_mut(id) {
                Ok(Entry::RegisteredPlayerStateEvent(stored)) => stored.last = current,
                Ok(_) => return Err(self.inner.error(REGISTER, Failure::WrongFamily)),
                Err(e) => return Err(self.inner.error(REGISTER, e.failure)),
            }
        }
        Ok(fired)
    }
}

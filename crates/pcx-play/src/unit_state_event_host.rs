//! Source-bound unit-state subscriptions. Registration seeds the observed value.
//! A boundary poll reads each live value (life, maximum life, mana, maximum
//! mana) and queues one event when it changed since the last observation and the
//! selected comparison holds at the new value. `SetUnitState`, regeneration and
//! combat all write the same vitals record, so one poll observes each of them.
//! Whether the native fires on every qualifying change or only on the transition
//! into the comparison is not established; this follows the player-state rule.
use super::super::EventSubject;
use super::player_state_events::{Comparison, LIMIT_OPS};
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

pub(super) const REGISTER: &str = "TriggerRegisterUnitStateEvent";
const CONVERT_STATE: &str = "ConvertUnitState";
const CONVERT_LIMIT: &str = "ConvertLimitOp";
const CONVERT_EVENT: &str = "ConvertUnitEvent";
const LIMIT_EVENT: &str = "EVENT_UNIT_STATE_LIMIT";
const RESPONSE: &str = "GetEventUnitState";

/// Admitted registration state on the shared trigger arena.
pub(super) struct Admission {
    max_records: usize,
    retained: usize,
    ops: [i32; LIMIT_OPS.len()],
    event: HandleId,
    registrations: Vec<HandleId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitStateEventRegistration {
    pub trigger: HandleId,
    pub unit: HandleId,
    pub state: HandleId,
    pub state_value: i32,
    pub limitop_value: i32,
    pub limit: f32,
    /// Most recently observed value; seeded at registration without firing.
    pub last: f32,
}

impl ObjectHost {
    /// Admit `TriggerRegisterUnitStateEvent` on the existing trigger arena. Requires
    /// admitted trigger creation. Unit vitals are read at registration and poll time,
    /// so registration refuses until they are admitted. The `unitstate`, `limitop`
    /// and `unitevent` conversions and the event enumerand must share the native's source.
    pub fn with_unit_state_events(mut self, max_records: usize) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(REGISTER, reason);
        let admitted = self.admitted_triggers()?;
        if admitted.unit_state.is_some() {
            return Err(fail(Failure::TriggerState(
                "unit state events already admitted",
            )));
        }
        std::alloc::Layout::array::<UnitStateEventRegistration>(max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "unit", "unitstate", "limitop", "real"],
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
        if program
            .natives()
            .get("CreateTrigger")
            .is_none_or(|n| &n.source != source)
        {
            return Err(fail(Failure::Source));
        }
        for (native, kind) in [
            (CONVERT_STATE, "unitstate"),
            (CONVERT_LIMIT, "limitop"),
            (CONVERT_EVENT, "unitevent"),
        ] {
            Self::contract(&self.inner, native, &["integer"], kind, true, None)?;
            let conversion = self
                .inner
                .conversions
                .get(native)
                .ok_or_else(|| fail(Failure::Signature))?;
            if conversion.kind != kind
                || &conversion.source != source
                || conversion.domain.is_empty()
            {
                return Err(self.inner.error(native, Failure::Source));
            }
        }
        for name in ["unitstate", "limitop", "unitevent"] {
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
            Self::contract(&self.inner, RESPONSE, &[], "unitstate", true, None)?;
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
                "unit state limit operations are not distinct",
            )));
        }
        let event_enumerand = self.source_constant_enumerand(LIMIT_EVENT, "unitevent")?;
        let mut registrations = Vec::new();
        registrations
            .try_reserve_exact(max_records)
            .map_err(|_| self.inner.error(REGISTER, Failure::Capacity))?;
        let event = match self
            .inner
            .convert_for(CONVERT_EVENT, &[Value::Int(event_enumerand)])
            .map_err(|e| self.inner.error(LIMIT_EVENT, e.failure))?
        {
            Value::Handle(event) => event,
            _ => return Err(self.inner.error(REGISTER, Failure::Signature)),
        };
        self.triggers
            .as_mut()
            .ok_or_else(|| {
                self.inner
                    .error(REGISTER, Failure::TriggerState("not admitted"))
            })?
            .unit_state = Some(Admission {
            max_records,
            retained: 0,
            ops,
            event,
            registrations,
        });
        Ok(self)
    }

    pub fn retained_unit_state_events(&self) -> Result<usize, Error> {
        Ok(self.unit_state_admission()?.retained)
    }

    pub fn unit_state_event_registration(
        &self,
        id: HandleId,
    ) -> Result<UnitStateEventRegistration, Error> {
        self.retained_unit_state_events()?;
        self.unit_state_record(id)
    }

    fn unit_state_admission(&self) -> Result<&Admission, Error> {
        self.triggers
            .as_ref()
            .and_then(|admitted| admitted.unit_state.as_ref())
            .ok_or_else(|| {
                self.inner.error(
                    REGISTER,
                    Failure::TriggerState("unit state events not admitted"),
                )
            })
    }

    fn unit_state_record(&self, id: HandleId) -> Result<UnitStateEventRegistration, Error> {
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(REGISTER, e.failure))?
        {
            Entry::RegisteredUnitStateEvent(record) => Ok(*record),
            _ => Err(self.inner.error(REGISTER, Failure::WrongFamily)),
        }
    }

    pub(super) fn register_unit_state_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(REGISTER, reason);
        let [
            Value::Handle(trigger),
            Value::Handle(unit),
            Value::Handle(state),
            Value::Handle(limitop),
            limit,
        ] = args
        else {
            return Err(fail(Failure::TriggerState(
                "expected trigger, unit, unit state, limitop and real threshold",
            )));
        };
        if !matches!(
            self.inner.entry(*trigger).map_err(|e| fail(e.failure))?,
            Entry::Trigger { .. }
        ) || !matches!(
            self.inner.entry(*unit).map_err(|e| fail(e.failure))?,
            Entry::Unit { .. }
        ) {
            return Err(fail(Failure::WrongFamily));
        }
        let state_value = self
            .inner
            .symbolic_value(*state, "unitstate")
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
        if !self.unit_state_admission()?.ops.contains(&limitop_value) {
            return Err(fail(Failure::OutsideDomain(limitop_value)));
        }
        let limit = limit
            .as_real()
            .filter(|v| v.is_finite())
            .ok_or_else(|| fail(Failure::TriggerState("finite real threshold required")))?;
        let last = self
            .unit_state_value(*unit, state_value)?
            .ok_or_else(|| fail(Failure::TriggerState("removed unit state is unsupported")))?;
        let admitted = self.unit_state_admission()?;
        if admitted.retained >= admitted.max_records {
            return Err(fail(Failure::Budget("unit state event registration")));
        }
        let record = UnitStateEventRegistration {
            trigger: *trigger,
            unit: *unit,
            state: *state,
            state_value,
            limitop_value,
            limit,
            last,
        };
        let id = self
            .inner
            .allocate(Entry::RegisteredUnitStateEvent(record), REGISTER)?;
        let admitted = self
            .triggers
            .as_mut()
            .and_then(|admitted| admitted.unit_state.as_mut())
            .expect("checked above");
        admitted.retained += 1;
        admitted.registrations.push(id);
        Ok(Value::Handle(id))
    }

    /// Queue one event per registration whose live value changed and whose
    /// comparison holds at the new value. Removed units are skipped.
    pub fn poll_unit_state_events(&mut self, work: &mut usize) -> Result<usize, Error> {
        let Some(admitted) = self.triggers.as_ref().and_then(|t| t.unit_state.as_ref()) else {
            return Ok(0);
        };
        let (ops, event, registrations) =
            (admitted.ops, admitted.event, admitted.registrations.len());
        let mut fired = 0;
        for slot in 0..registrations {
            *work = work.checked_sub(1).ok_or_else(|| {
                self.inner
                    .error(REGISTER, Failure::Budget("unit state scan"))
            })?;
            let id = self.unit_state_admission()?.registrations[slot];
            let record = self.unit_state_record(id)?;
            let Some(current) = self.unit_state_value(record.unit, record.state_value)? else {
                continue;
            };
            if current == record.last {
                continue;
            }
            let index = ops
                .iter()
                .position(|value| *value == record.limitop_value)
                .ok_or_else(|| {
                    self.inner
                        .error(REGISTER, Failure::OutsideDomain(record.limitop_value))
                })?;
            let comparison = Comparison::at(index)
                .ok_or_else(|| self.inner.error(REGISTER, Failure::Signature))?;
            if comparison.holds(current, record.limit) {
                self.queue_event(
                    record.trigger,
                    None,
                    EventSubject {
                        unit: Some(record.unit),
                        unit_state: Some(record.state),
                        ..Default::default()
                    },
                    Some(event),
                )?;
                fired += 1;
            }
            match self.inner.entry_mut(id) {
                Ok(Entry::RegisteredUnitStateEvent(stored)) => stored.last = current,
                Ok(_) => return Err(self.inner.error(REGISTER, Failure::WrongFamily)),
                Err(e) => return Err(self.inner.error(REGISTER, e.failure)),
            }
        }
        Ok(fired)
    }
}

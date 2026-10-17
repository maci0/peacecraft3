//! Timer-expiry subscriptions on existing identities, without a second clock.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

pub(super) const REGISTER: &str = "TriggerRegisterTimerExpireEvent";

/// Retained subscription, not a queued expiry or proof that its timer remains live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerEventRegistration {
    pub trigger: HandleId,
    pub timer: HandleId,
}

impl ObjectHost {
    /// Admit the loaded trigger/timer -> event contract on the existing arena.
    /// No timeout, running state, enabled default or callback delivery is inferred.
    pub fn with_timer_events(mut self, max_records: usize) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(REGISTER, reason);
        let admitted = self.triggers.as_ref().ok_or_else(|| {
            fail(Failure::TriggerState(
                "trigger creation must already be admitted",
            ))
        })?;
        if admitted.timer_event_limit.is_some() {
            return Err(fail(Failure::TriggerState("timer events already admitted")));
        }
        std::alloc::Layout::array::<TimerEventRegistration>(max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "timer"],
            "event",
            false,
            Some("event"),
        )?;
        let program = &self.inner.program;
        let registration = program
            .natives()
            .get(REGISTER)
            .ok_or_else(|| fail(Failure::Signature))?;
        for name in ["CreateTimer", "CreateTrigger"] {
            if program
                .natives()
                .get(name)
                .is_none_or(|n| n.source != registration.source)
            {
                return Err(fail(Failure::Source));
            }
        }
        let mut kind = "timer";
        for _ in 0..=program.types().count() {
            if kind == "handle" {
                break;
            }
            let ty = program
                .types()
                .find(|ty| ty.name == kind)
                .ok_or_else(|| fail(Failure::Signature))?;
            if ty.source != registration.source {
                return Err(fail(Failure::Source));
            }
            kind = &ty.parent;
        }
        if kind != "handle" {
            return Err(fail(Failure::Signature));
        }
        self.triggers
            .as_mut()
            .ok_or_else(|| fail(Failure::TriggerState("not admitted")))?
            .timer_event_limit = Some(max_records);
        Ok(self)
    }

    /// Historical association. Destroying the timer does not make it deliverable
    /// or recycle either token; a delivery consumer must validate current lifetimes.
    pub fn timer_event_registration(&self, id: HandleId) -> Result<TimerEventRegistration, Error> {
        self.retained_timer_events()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(REGISTER, e.failure))?
        {
            Entry::RegisteredTimerEvent { trigger, timer } => Ok(TimerEventRegistration {
                trigger: *trigger,
                timer: *timer,
            }),
            _ => Err(self.inner.error(REGISTER, Failure::WrongFamily)),
        }
    }

    pub fn retained_timer_events(&self) -> Result<usize, Error> {
        let admitted = self
            .triggers
            .as_ref()
            .filter(|a| a.timer_event_limit.is_some())
            .ok_or_else(|| {
                self.inner
                    .error(REGISTER, Failure::TriggerState("timer events not admitted"))
            })?;
        Ok(admitted.retained_timer_events)
    }

    pub(super) fn register_timer_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(REGISTER, reason);
        let [Value::Handle(trigger), Value::Handle(timer)] = args else {
            return Err(fail(Failure::TriggerState(
                "expected trigger and timer identities",
            )));
        };
        if !matches!(
            self.inner.entry(*trigger).map_err(|e| fail(e.failure))?,
            Entry::Trigger { .. }
        ) {
            return Err(fail(Failure::WrongFamily));
        }
        match self.inner.entry(*timer).map_err(|e| fail(e.failure))? {
            Entry::Timer { live: true, .. } => {}
            Entry::Timer { live: false, .. } => return Err(fail(Failure::Identity)),
            _ => return Err(fail(Failure::WrongFamily)),
        }
        let admitted = self
            .triggers
            .as_ref()
            .ok_or_else(|| fail(Failure::TriggerState("not admitted")))?;
        let limit = admitted
            .timer_event_limit
            .ok_or_else(|| fail(Failure::TriggerState("timer events not admitted")))?;
        if admitted.retained_timer_events >= limit {
            return Err(fail(Failure::Budget("timer event registration")));
        }
        let id = self.inner.allocate(
            Entry::RegisteredTimerEvent {
                trigger: *trigger,
                timer: *timer,
            },
            REGISTER,
        )?;
        let admitted = self.triggers.as_mut().ok_or_else(|| {
            self.inner
                .error(REGISTER, Failure::TriggerState("not admitted"))
        })?;
        admitted.retained_timer_events += 1;
        Ok(Value::Handle(id))
    }
}

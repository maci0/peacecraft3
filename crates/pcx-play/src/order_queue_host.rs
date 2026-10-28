//! Shift-queued unit orders.
//!
//! A queued order waits behind the unit's current order and is issued, through the same
//! order natives a script uses, on the first update at which the unit reports no current
//! order and is not holding position. A queued order that the unit then refuses is
//! dropped. Any order issued to the unit by another route (the player without Shift, a
//! script, a stop) replaces the queue: the client sends the Shift state as bit 0 of the
//! order flags (`0x5fd2f0`) and an unflagged order replaces the unit's orders.
//!
//! The length the client allows a queue to grow to is not established. The bounds here
//! are the host's resource bounds and the caller's admission decides them.
use std::collections::{BTreeMap, VecDeque};

use pcx_script::{HandleId, Host, Value};

use super::smart_order::OrderForm;
use super::{Error, ObjectHost};

/// Resource bounds of the order queues, not original game rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueBudget {
    /// Most units with a waiting order.
    pub units: usize,
    /// Most waiting orders per unit.
    pub orders_per_unit: usize,
}

/// What `queue_order` did with an order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueOutcome {
    /// The unit was idle, so the order was issued at once; the flag is the native's answer.
    Issued(bool),
    /// The order waits at this position behind the current order (1 is next).
    Queued(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum QueueRefusal {
    #[error("order queueing is not admitted")]
    NotAdmitted,
    #[error("the unit already has {limit} queued orders")]
    UnitFull { limit: usize },
    #[error("{limit} units already have queued orders")]
    Full { limit: usize },
    #[error("{0}")]
    Host(String),
}

#[derive(Debug)]
pub(super) struct State {
    budget: QueueBudget,
    queues: BTreeMap<HandleId, VecDeque<OrderForm>>,
    /// Set while the queue itself issues, so its own order does not clear the queue.
    issuing: bool,
    refused: Vec<String>,
}

impl ObjectHost {
    /// Admit order queues once. Both bounds must be positive.
    pub fn admit_order_queue(&mut self, budget: QueueBudget) -> Result<(), Error> {
        if self.order_queue.is_some() || budget.units == 0 || budget.orders_per_unit == 0 {
            return Err(self.inner.error(
                "QueueOrder",
                super::Failure::UnitActivity("admit order queues once with positive bounds".into()),
            ));
        }
        self.order_queue = Some(State {
            budget,
            queues: BTreeMap::new(),
            issuing: false,
            refused: Vec::new(),
        });
        Ok(())
    }

    /// Orders waiting for `unit`.
    pub fn queued_orders(&self, unit: HandleId) -> Vec<OrderForm> {
        self.order_queue
            .as_ref()
            .and_then(|state| state.queues.get(&unit))
            .map(|queue| queue.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Why orders were dropped from the queues since the last call.
    pub fn take_queue_refusals(&mut self) -> Vec<String> {
        self.order_queue
            .as_mut()
            .map(|state| std::mem::take(&mut state.refused))
            .unwrap_or_default()
    }

    /// Issues `form` now when the unit has nothing to do, otherwise appends it to the
    /// unit's queue.
    pub fn queue_order(
        &mut self,
        unit: HandleId,
        form: OrderForm,
    ) -> Result<QueueOutcome, QueueRefusal> {
        let state = self.order_queue.as_ref().ok_or(QueueRefusal::NotAdmitted)?;
        let waiting = state.queues.get(&unit).map_or(0, VecDeque::len);
        let (budget, queued_units) = (state.budget, state.queues.len());
        if waiting == 0 && self.unit_order_idle(unit).map_err(QueueRefusal::Host)? {
            return self
                .issue_order_form(unit, form)
                .map(QueueOutcome::Issued)
                .map_err(QueueRefusal::Host);
        }
        if waiting >= budget.orders_per_unit {
            return Err(QueueRefusal::UnitFull {
                limit: budget.orders_per_unit,
            });
        }
        if waiting == 0 && queued_units >= budget.units {
            return Err(QueueRefusal::Full {
                limit: budget.units,
            });
        }
        let queue = self
            .order_queue
            .as_mut()
            .expect("checked above")
            .queues
            .entry(unit)
            .or_default();
        queue.push_back(form);
        Ok(QueueOutcome::Queued(queue.len()))
    }

    /// Whether the unit reports no order and is not holding position.
    fn unit_order_idle(&mut self, unit: HandleId) -> Result<bool, String> {
        let held = self
            .executed_unit(unit)
            .map_err(|error| error.to_string())?
            .activity
            .as_ref()
            .is_some_and(|activity| activity.held());
        if held {
            return Ok(false);
        }
        match self.native("GetUnitCurrentOrder", &[Value::Handle(unit)])? {
            Value::Int(0) => Ok(true),
            Value::Int(_) => Ok(false),
            other => Err(format!("GetUnitCurrentOrder returned {other:?}")),
        }
    }

    /// An order issued to a unit by any route but the queue replaces its queue.
    pub(super) fn order_queue_observe(&mut self, name: &str, args: &[Value]) {
        let Some(state) = self.order_queue.as_mut() else {
            return;
        };
        if state.issuing || state.queues.is_empty() {
            return;
        }
        if name.starts_with("Issue")
            && name.contains("Order")
            && let Some(Value::Handle(unit)) = args.first()
        {
            state.queues.remove(unit);
        }
    }

    /// Issues the next waiting order of each unit that has become idle.
    pub(super) fn advance_order_queue(&mut self) {
        let Some(state) = self.order_queue.as_ref() else {
            return;
        };
        let units: Vec<HandleId> = state.queues.keys().copied().collect();
        for unit in units {
            let (gone, paused) = match self.executed_unit(unit) {
                Err(_) => (true, false),
                Ok(record) => (
                    record.combat.as_ref().is_some_and(|combat| combat.dead),
                    record.paused_since.is_some(),
                ),
            };
            if gone {
                self.order_queue
                    .as_mut()
                    .expect("admitted")
                    .queues
                    .remove(&unit);
            }
            if gone || paused {
                continue;
            }
            match self.unit_order_idle(unit) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => {
                    self.drop_queue(unit, error);
                    continue;
                }
            }
            let Some(form) = self
                .order_queue
                .as_mut()
                .and_then(|state| state.queues.get_mut(&unit))
                .and_then(VecDeque::pop_front)
            else {
                continue;
            };
            let state = self.order_queue.as_mut().expect("admitted");
            if state.queues.get(&unit).is_some_and(VecDeque::is_empty) {
                state.queues.remove(&unit);
            }
            state.issuing = true;
            let result = self.issue_order_form(unit, form);
            let state = self.order_queue.as_mut().expect("admitted");
            state.issuing = false;
            match result {
                Ok(true) => {}
                Ok(false) => state.refused.push(format!(
                    "queued order {} was refused by the unit",
                    form.order()
                )),
                Err(error) => state.refused.push(error),
            }
        }
    }

    fn drop_queue(&mut self, unit: HandleId, reason: String) {
        let state = self.order_queue.as_mut().expect("admitted");
        state.queues.remove(&unit);
        state.refused.push(reason);
    }
}
